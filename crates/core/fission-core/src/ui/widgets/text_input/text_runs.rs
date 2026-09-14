//! Offset clamping and styled-run splitting for text input painting.

pub(super) fn clamp_text_offset(value: &str, mut offset: usize) -> usize {
    offset = offset.min(value.len());
    while offset > 0 && !value.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

pub(super) fn split_runs_for_range(
    runs: &[fission_ir::op::TextRun],
    start: usize,
    end: usize,
    mut apply: impl FnMut(&mut fission_ir::op::TextStyle),
) -> Vec<fission_ir::op::TextRun> {
    if start >= end {
        return runs.to_vec();
    }

    let mut out = Vec::new();
    let mut run_start = 0usize;
    for run in runs {
        let run_end = run_start + run.text.len();
        let overlap_start = start.max(run_start);
        let overlap_end = end.min(run_end);
        if overlap_start >= overlap_end {
            out.push(run.clone());
            run_start = run_end;
            continue;
        }

        let local_start = overlap_start - run_start;
        let local_end = overlap_end - run_start;
        if local_start > 0 {
            out.push(fission_ir::op::TextRun {
                text: run.text[..local_start].to_string(),
                style: run.style.clone(),
            });
        }

        let mut styled = run.style.clone();
        apply(&mut styled);
        out.push(fission_ir::op::TextRun {
            text: run.text[local_start..local_end].to_string(),
            style: styled,
        });

        if local_end < run.text.len() {
            out.push(fission_ir::op::TextRun {
                text: run.text[local_end..].to_string(),
                style: run.style.clone(),
            });
        }

        run_start = run_end;
    }
    out
}
