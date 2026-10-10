"""Exercise custom title bars using real GNOME/Wayland input and compositor state.

Run in an isolated GNOME test session with its developer console enabled for
window observation (see docs/testing/custom-title-bar.md).
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import time
import urllib.request
from gi.repository import Gio, GLib

parser = argparse.ArgumentParser()
parser.add_argument('--binary', required=True)
parser.add_argument('--output', required=True)
parser.add_argument('--session-bus-address-file')
args = parser.parse_args()
if args.session_bus_address_file:
    os.environ['DBUS_SESSION_BUS_ADDRESS'] = Path(args.session_bus_address_file).read_text()
out = Path(args.output)
out.mkdir(parents=True, exist_ok=True)
bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
remote_dest = 'org.gnome.Mutter.RemoteDesktop'
remote = bus.call_sync(remote_dest, '/org/gnome/Mutter/RemoteDesktop', remote_dest,
                       'CreateSession', None, None, Gio.DBusCallFlags.NONE, 10000, None).unpack()[0]

def remote_call(method, signature=None, *values):
    return bus.call_sync(remote_dest, remote, remote_dest+'.Session', method,
                         GLib.Variant(signature, values) if signature else None,
                         None, Gio.DBusCallFlags.NONE, 10000, None)

def shell_eval(script):
    success, result = bus.call_sync('org.gnome.Shell', '/org/gnome/Shell', 'org.gnome.Shell',
                                    'Eval', GLib.Variant('(s)', (script,)), None,
                                    Gio.DBusCallFlags.NONE, 10000, None).unpack()
    if not success:
        raise RuntimeError('GNOME window observation unavailable: enable its developer console in the isolated test session')
    return json.loads(result) if result else None

def window_script(body):
    return f'(() => {{ const w = global.get_window_actors().map(a => a.meta_window).find(w => w.get_pid() === {app.pid}); if (!w) return null; {body} }})()'

def window_state():
    return shell_eval(window_script('const r = w.get_frame_rect(); return {focused:w.has_focus(),x:r.x,y:r.y,width:r.width,height:r.height,decorated:w.decorated,maximized:w.maximized_horizontally && w.maximized_vertically,minimized:w.minimized};'))

def wait_state(predicate, timeout=10):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        state = window_state()
        if state and predicate(state):
            return state
        time.sleep(.1)
    raise AssertionError(f'native window state did not settle: {window_state()}')

def key(symbol, pressed):
    remote_call('NotifyKeyboardKeysym', '(ub)', symbol, pressed)

def tap_key(symbol):
    key(symbol, True)
    key(symbol, False)
    time.sleep(.1)

def move_pointer(x, y):
    current = shell_eval('global.get_pointer().slice(0, 2)')
    remote_call('NotifyPointerMotionRelative', '(dd)', x-current[0], y-current[1])
    time.sleep(.1)

def mouse(pressed):
    remote_call('NotifyPointerButton', '(ib)', 272, pressed)
    time.sleep(.08)

def click(x, y):
    move_pointer(x, y)
    mouse(True)
    mouse(False)

def drag(x, y, dx, dy):
    move_pointer(x, y)
    mouse(True)
    remote_call('NotifyPointerMotionRelative', '(dd)', dx, dy)
    time.sleep(.25)
    mouse(False)

with socket.socket() as reservation:
    reservation.bind(('127.0.0.1', 0))
    port = reservation.getsockname()[1]
env = dict(os.environ, FISSION_TEST_CONTROL_PORT=str(port), FISSION_RENDERER='software',
           FISSION_TEXTINPUT_BLINK='0')
env.pop('FISSION_BACKGROUND_TEST', None)
env.pop('DISPLAY', None)

def command(cmd, **fields):
    request = urllib.request.Request(f'http://127.0.0.1:{port}/cmd',
        json.dumps(dict(cmd=cmd, **fields)).encode(), {'Content-Type':'application/json'})
    with urllib.request.urlopen(request, timeout=10) as response:
        result = json.load(response)
    if result.get('status') in ['Error', 'SelectorError']:
        raise RuntimeError(result)
    return result

def node(identifier):
    matches = [n for n in command('GetTree')['nodes'] if n.get('identifier') == identifier]
    assert len(matches) == 1, (identifier, matches)
    return matches[0]

def click_control(identifier):
    # Force a native enter/motion after a compositor move/resize as well as a
    # capability change; a stationary newly created virtual seat has no motion.
    move_pointer(20, 16)
    control = node(identifier)
    frame = window_state()
    click(frame['x']+control['x']+control['width']/2, frame['y']+control['y']+control['height']/2)

def title_point():
    region = node('window.drag')
    frame = window_state()
    # The left title text is a noninteractive descendant of the drag region.
    return frame['x']+region['x']+24, frame['y']+region['y']+region['height']/2

report = {'binary_sha256': hashlib.sha256(Path(args.binary).read_bytes()).hexdigest(),
          'os': Path('/etc/os-release').read_text(), 'states':{}, 'checks':[]}
with (out/'app.log').open('wb') as log:
    remote_call('Start')
    app = subprocess.Popen([args.binary], env=env, stdout=log, stderr=log)
    try:
        shell_eval('1')
        deadline = time.monotonic() + 60
        while True:
            if app.poll() is not None:
                raise RuntimeError(f'app exited {app.returncode}')
            try:
                node('window.drag')
                break
            except (OSError, KeyError, AssertionError):
                pass
            if time.monotonic() > deadline:
                raise RuntimeError('app did not become ready')
            time.sleep(.2)
        # Dismiss the isolated compositor's initial overview/search before app input.
        tap_key(0xff1b)
        tap_key(0xff1b)
        move_pointer(20, 80)
        shell_eval(window_script('w.activate(global.get_current_time()); return true;'))
        initial = wait_state(lambda s: not s['maximized'] and s['focused'])
        assert initial['decorated'] is False, initial
        report['states']['initial'] = initial
        command('Screenshot', path=str(out/'initial.png'))
        report['checks'].append('native decorations hidden')

        click_control('window.note')
        for character in 'hello':
            tap_key(ord(character))
        command('Screenshot', path=str(out/'after-typing.png'))
        command('WaitForText', text='hello', timeout_ms=5000)
        assert node('window.note')['value'] == 'hello'
        assert window_state() == initial, 'typing in the title bar moved the window'
        report['checks'].append('native title-bar typing without window dragging')
        click_control('window.increment')
        command('WaitForText', text='1', timeout_ms=5000)
        report['checks'].append('ordinary app button')

        # Increment is last in tab order: two backward steps reach Maximize.
        key(0xffe1, True)
        tap_key(0xff09)
        tap_key(0xff09)
        key(0xffe1, False)
        tap_key(0xff0d)
        report['states']['maximized'] = wait_state(lambda s: s['maximized'])
        command('WaitForText', text='Window maximized', timeout_ms=5000)
        assert node('window.maximize')['label'] == 'Restore'
        command('Screenshot', path=str(out/'maximized.png'))
        click_control('window.maximize')
        restored = wait_state(lambda s: not s['maximized'])
        command('WaitForText', text='Window restored', timeout_ms=5000)
        report['checks'].append('keyboard maximize, observed state, and native restore button')

        x, y = title_point()
        drag(x, y, 30, 10)
        moved = wait_state(lambda s: (s['x'],s['y']) != (restored['x'],restored['y']))
        report['states']['moved'] = moved
        assert (moved['width'],moved['height']) == (restored['width'],restored['height'])
        report['checks'].append('native compositor move from title-bar press')

        time.sleep(.6)
        x, y = title_point()
        click(x, y)
        click(x, y)
        wait_state(lambda s: s['maximized'])
        time.sleep(.6)
        x, y = title_point()
        click(x, y)
        click(x, y)
        unmaximized = wait_state(lambda s: not s['maximized'])
        report['checks'].append('native double-click maximize and restore')

        drag(unmaximized['x']+unmaximized['width']-2,
             unmaximized['y']+unmaximized['height']-2, 40, 20)
        resized = wait_state(lambda s: s['width'] > unmaximized['width'] and s['height'] > unmaximized['height'])
        report['states']['resized'] = resized
        command('Screenshot', path=str(out/'resized.png'))
        report['checks'].append('native borderless corner resize')

        click_control('window.minimize')
        report['states']['minimized'] = wait_state(lambda s: s['minimized'])
        # Wayland restoration from minimization belongs to the compositor/task switcher.
        shell_eval(window_script('w.unminimize(); w.activate(global.get_current_time()); return true;'))
        wait_state(lambda s: not s['minimized'])
        report['checks'].append('native minimize button and compositor restoration')
        click_control('window.close')
        app.wait(timeout=10)
        assert app.returncode == 0, app.returncode
        report['checks'].append('native close button and bounded successful shutdown')
        report['passed'] = True
    except Exception as error:
        report['failure'] = str(error)
        try:
            report['tree_at_failure'] = command('GetTree')
            command('Screenshot', path=str(out/'failure.png'))
        except Exception:
            pass
        raise
    finally:
        if app.poll() is None:
            app.kill()
            app.wait(timeout=5)
        try:
            remote_call('Stop')
        finally:
            (out/'results.json').write_text(json.dumps(report, indent=2))
print(json.dumps(report, indent=2))
