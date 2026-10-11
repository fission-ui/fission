"""Measure the reporter's visible native app in an Ubuntu Wayland session."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import time
import urllib.request

parser = argparse.ArgumentParser()
parser.add_argument('--binary', required=True)
parser.add_argument('--output', required=True)
parser.add_argument('--blink', choices=['0', '1'], required=True)
parser.add_argument('--renderer', default='auto')
parser.add_argument('--gnome', action='store_true')
parser.add_argument('--session-bus-address-file')
parser.add_argument('--record-baseline', action='store_true')
parser.add_argument('--samples', type=int, default=2)
args = parser.parse_args()
if args.samples < 1:
    parser.error('--samples must be positive')
out = Path(args.output)
out.mkdir(parents=True, exist_ok=True)
with socket.socket() as reservation:
    reservation.bind(('127.0.0.1', 0))
    port = reservation.getsockname()[1]
log = out / 'frames.log'
env = dict(os.environ, FISSION_TEST_CONTROL_PORT=str(port),
           FISSION_FRAME_TRACE='1', FISSION_TEXTINPUT_BLINK=args.blink,
           FISSION_RENDERER=args.renderer, FISSION_TEXTINPUT_BLINK_MS='530')
env.pop('FISSION_BACKGROUND_TEST', None)
env.pop('DISPLAY', None)
remote = None
if args.gnome:
    from gi.repository import Gio, GLib
    if args.session_bus_address_file:
        os.environ['DBUS_SESSION_BUS_ADDRESS'] = Path(args.session_bus_address_file).read_text()
    env['DBUS_SESSION_BUS_ADDRESS'] = os.environ['DBUS_SESSION_BUS_ADDRESS']
    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    dest = 'org.gnome.Mutter.RemoteDesktop'
    remote = bus.call_sync(dest, '/org/gnome/Mutter/RemoteDesktop', dest, 'CreateSession',
                           None, None, Gio.DBusCallFlags.NONE, 10000, None).unpack()[0]
    def remote_call(method, signature=None, *values):
        return bus.call_sync(dest, remote, dest+'.Session', method,
                             GLib.Variant(signature, values) if signature else None,
                             None, Gio.DBusCallFlags.NONE, 10000, None)
    remote_call('Start')

def native_key(keysym):
    remote_call('NotifyKeyboardKeysym', '(ub)', keysym, True)
    remote_call('NotifyKeyboardKeysym', '(ub)', keysym, False)
    time.sleep(.1)

def command(cmd, **fields):
    body = json.dumps(dict(cmd=cmd, **fields)).encode()
    request = urllib.request.Request(f'http://127.0.0.1:{port}/cmd', body,
                                     {'Content-Type': 'application/json'})
    with urllib.request.urlopen(request, timeout=10) as response:
        result = json.load(response)
    if result['status'] in ['Error', 'SelectorError']:
        raise RuntimeError(result)
    return result

def query(label):
    return {'selector': {'kind': 'label', 'label': label}}

def ticks(pid):
    fields = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
    return int(fields[11]) + int(fields[12])

def threads(pid):
    result = []
    for task in Path(f'/proc/{pid}/task').iterdir():
        try:
            stat = (task / 'stat').read_text().rsplit(')', 1)[1].split()
            result.append({'tid': int(task.name), 'name': (task/'comm').read_text().strip(),
                           'ticks': int(stat[11])+int(stat[12]),
                           'state': stat[0], 'wchan': (task/'wchan').read_text().strip()})
        except FileNotFoundError:
            pass
    return result

def sample(app, phase):
    time.sleep(3)
    samples = []
    for index in range(args.samples):
        offset = log.stat().st_size
        before = ticks(app.pid)
        before_threads = {t['tid']: t for t in threads(app.pid)}
        started = time.monotonic()
        time.sleep(10)
        elapsed = time.monotonic() - started
        after = ticks(app.pid)
        text = log.read_bytes()[offset:].decode(errors='replace')
        thread_results = threads(app.pid)
        for thread in thread_results:
            old = before_threads.get(thread['tid'])
            thread['delta_ticks'] = thread['ticks'] - old['ticks'] if old else None
        samples.append({'index': index, 'elapsed_seconds': elapsed,
                        'cpu_percent_one_core': (after-before)/os.sysconf('SC_CLK_TCK')/elapsed*100,
                        'redraws': text.count('phase=redraw_requested'),
                        'event_loop_waits': text.count('phase=about_to_wait'),
                        'threads': thread_results})
    result = {'phase': phase, 'samples': samples}
    print(json.dumps({'phase': phase, 'samples': [{k:v for k,v in s.items() if k != 'threads'} for s in samples]}), flush=True)
    return result

with log.open('wb') as output:
    app = subprocess.Popen([args.binary], env=env, stdout=output, stderr=output)
    try:
        deadline = time.monotonic() + 60
        while True:
            if app.poll() is not None:
                raise RuntimeError(f'app exited {app.returncode}: {log.read_text()[-2000:]}')
            try:
                tree = command('GetTree')
                if any(n['label'] == 'Title' for n in tree.get('nodes', [])):
                    break
            except OSError:
                pass
            if time.monotonic() > deadline:
                raise RuntimeError('app did not become ready')
            time.sleep(.2)
        phases = [sample(app, 'unfocused')]
        # Keep the compositor keyboard seat alive separately; focus uses LiveTest.
        command('FocusSelector', query=query('Title'))
        command('Pump')
        if args.gnome:
            # A native navigation event activates GNOME's lazy input context.
            # It also verifies real cursor movement without changing app text.
            native_key(0xff57)  # End
            time.sleep(.5)
            native_key(0xff57)
        time.sleep(.5)
        focused_log = log.read_text()
        if 'caret_focus_changed' not in focused_log:
            raise RuntimeError('input focus was not applied')
        phases.append(sample(app, 'focused_idle'))
        if args.gnome:
            for letter in 'hello':
                native_key(ord(letter))
        else:
            subprocess.run(['wtype', '-d', '100', 'hello'], env=env, check=True, timeout=10)
        time.sleep(.5)
        text = command('GetText')
        if not any('hello' in item['text'] for item in text['items']):
            raise RuntimeError(f'native typing not visible: {text}')
        command('Screenshot', path=str(out/'focused.png'))
        command('FocusSelector', query=query('Increment'))
        command('Pump')
        phases.append(sample(app, 'button_focused'))
        command('ActivateSelector', query=query('Increment'))
        command('ActivateSelector', query=query('Increment by 4'))
        command('WaitForText', text='Count: 5', timeout_ms=5000)
        command('FocusSelector', query=query('Title'))
        command('Pump')
        native_composition = False
        if args.gnome:
            native_key(0xff57)
            time.sleep(.5)
            # GNOME/IBus Unicode input exercises actual platform preedit/commit.
            remote_call('NotifyKeyboardKeysym', '(ub)', 0xffe3, True)  # Ctrl
            remote_call('NotifyKeyboardKeysym', '(ub)', 0xffe1, True)  # Shift
            native_key(ord('u'))
            remote_call('NotifyKeyboardKeysym', '(ub)', 0xffe1, False)
            remote_call('NotifyKeyboardKeysym', '(ub)', 0xffe3, False)
            native_key(ord('e'))
            native_key(ord('9'))
            native_key(0xff0d)  # Return commits U+00E9.
            command('WaitForText', text='é', timeout_ms=5000)
            native_composition = True
            def input_node():
                return next(n for n in command('GetTree')['nodes'] if n['label'] == 'Title')
            native_key(0xff57)
            end = input_node()['text_selection']
            remote_call('NotifyKeyboardKeysym', '(ub)', 0xffe1, True)
            native_key(0xff51)  # Shift+Left selects the preceding character.
            remote_call('NotifyKeyboardKeysym', '(ub)', 0xffe1, False)
            selected = input_node()['text_selection']
            assert selected[0] != selected[1], f'native selection failed: {selected}'
            native_key(0xff53)  # Right collapses selection at the end.
            assert input_node()['text_selection'] == end, 'native selection did not collapse'
            committed = input_node()['value']
            remote_call('NotifyKeyboardKeysym', '(ub)', 0xffe3, True)
            remote_call('NotifyKeyboardKeysym', '(ub)', 0xffe1, True)
            native_key(ord('u'))
            remote_call('NotifyKeyboardKeysym', '(ub)', 0xffe1, False)
            remote_call('NotifyKeyboardKeysym', '(ub)', 0xffe3, False)
            native_key(ord('e'))
            native_key(0xff1b)  # Escape cancels native composition.
            time.sleep(.5)
            assert input_node()['value'] == committed, 'cancelled composition changed committed text'
        if args.gnome:
            native_key(0xff57)
            before_injected_commit = input_node()['value']
        command('ImePreedit', text='x', cursor_start=0, cursor_end=1)
        command('ImeCommit', text='é')
        command('WaitForText', text=before_injected_commit + 'é' if args.gnome else 'é', timeout_ms=5000)
        command('Quit')
        app.wait(timeout=10)
        if app.returncode:
            raise RuntimeError(f'app exited {app.returncode}')
        report = {'binary': args.binary, 'blink': args.blink, 'renderer_request': args.renderer,
                  'binary_sha256': hashlib.sha256(Path(args.binary).read_bytes()).hexdigest(),
                  'compositor': 'GNOME 49' if args.gnome else 'Sway',
                  'os': Path('/etc/os-release').read_text(),
                  'renderer_line': next((l for l in log.read_text().splitlines() if 'renderer:' in l), None),
                  'visible_window': True, 'native_wayland_typing_verified': True, 'semantic_focus_verified': True,
                  'native_unicode_composition_verified': native_composition,
                  'native_selection_and_composition_cancel_verified': native_composition,
                  'counter_actions_verified': True, 'injected_ime_verified': True,
                  'phases': phases}
        (out/'results.json').write_text(json.dumps(report, indent=2))
        if not args.record_baseline:
            for phase in phases:
                limit = 25 if phase['phase'] == 'focused_idle' and args.blink == '1' else 2
                for result in phase['samples']:
                    assert result['redraws'] <= limit, f"{phase['phase']}: {result['redraws']} idle redraws exceed {limit}"
                    assert result['event_loop_waits'] <= 100, f"{phase['phase']}: {result['event_loop_waits']} idle wakeups exceed 100"
        print(f'Saved {out}/results.json', flush=True)
    finally:
        if app.poll() is None:
            app.kill()
            app.wait(timeout=10)
        if remote:
            remote_call('Stop')
