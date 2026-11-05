//! Flat CSV export: one row per set, with that session's strength for the variant.

use std::collections::HashMap;
use std::io::Write;

use crate::log::SetRow;
use crate::metrics::{self, Params, SessionStrength};

const HEADER: &[&str] = &[
    "block", "block_label", "split", "cycle_id", "week", "session_id", "performed_on",
    "day_label", "position", "variant", "variant_display", "family", "gym", "joint_action",
    "set_number", "logged", "weight", "weight_inferred", "reps", "rir_low", "rir_high",
    "rir_mid", "reps_to_failure", "set_e1rm", "hard", "session_e1rm", "session_e5rm",
    "session_pr", "comment",
];

/// Write every set as CSV. A missing RIR is left blank in `rir_low`/`rir_high` but
/// counts as 0 in `rir_mid` and the estimates.
pub fn write_csv(rows: &[SetRow], p: &Params, out: &mut impl Write) -> std::io::Result<usize> {
    let mut by_session: HashMap<(i64, i64), SessionStrength> = HashMap::new();
    for v in metrics::variants_in(rows) {
        for s in metrics::variant_history(rows, v.id, p) {
            by_session.insert((s.session_id, v.id), s);
        }
    }

    writeln!(out, "{}", HEADER.join(","))?;
    for r in rows {
        let set = &r.set;
        let session = r
            .variant_id
            .and_then(|v| by_session.get(&(r.session_id, v)));
        let fields = [
            r.block_slug.clone(),
            opt(&r.block_label),
            r.split.as_str().to_string(),
            r.cycle_id.to_string(),
            r.week.to_string(),
            r.session_id.to_string(),
            opt(&r.performed_on),
            opt(&r.day_label),
            r.position.to_string(),
            opt(&r.variant_slug),
            opt(&r.variant_display),
            opt(&r.family),
            opt(&r.gym),
            opt(&r.joint_action),
            set.set_number.to_string(),
            bool01(set.logged),
            num(set.weight),
            bool01(set.weight_inferred),
            num(set.reps),
            num(set.rir_low),
            num(set.rir_high),
            num(set.logged.then(|| metrics::rir_mid(set.rir_low, set.rir_high))),
            num(metrics::reps_to_failure(set)),
            num(metrics::set_e1rm(set, p.epley_c)),
            bool01(metrics::is_hard_set(set, p)),
            num(session.map(|s| s.e1rm)),
            num(session.map(|s| s.e5rm)),
            session.map(|s| bool01(s.pr)).unwrap_or_default(),
            opt(&set.comment),
        ];
        let line: Vec<String> = fields.iter().map(|f| quote(f)).collect();
        writeln!(out, "{}", line.join(","))?;
    }
    Ok(rows.len())
}

fn opt(v: &Option<String>) -> String {
    v.clone().unwrap_or_default()
}

fn num(v: Option<f64>) -> String {
    v.map(|x| {
        let s = format!("{x:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    })
    .unwrap_or_default()
}

fn bool01(v: bool) -> String {
    if v { "1" } else { "0" }.to_string()
}

fn quote(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::{SetLog, Split};

    fn row(session: i64, weight: f64, reps: f64, comment: Option<&str>) -> SetRow {
        SetRow {
            block_slug: "b".into(),
            block_label: Some("home, gym".into()),
            split: Split::FullBody,
            cycle_id: 1,
            week: 1,
            session_id: session,
            performed_on: Some("2026-01-01".into()),
            day_label: Some("FB 1".into()),
            position: 1,
            variant_id: Some(7),
            variant_slug: Some("curl".into()),
            variant_display: Some("Curl".into()),
            family: None,
            gym: None,
            joint_action: None,
            set: SetLog {
                id: 0,
                entry_id: 0,
                set_number: 1,
                weight: Some(weight),
                weight_inferred: false,
                weight_note: None,
                reps: Some(reps),
                rir_low: None,
                rir_high: None,
                logged: true,
                comment: comment.map(String::from),
                raw: None,
            },
        }
    }

    #[test]
    fn csv_has_one_row_per_set_with_session_strength() {
        let rows = vec![
            row(1, 100.0, 5.0, Some("said \"easy\"")),
            row(1, 90.0, 10.0, None),
        ];
        let mut buf = Vec::new();
        let n = write_csv(&rows, &Params::default(), &mut buf).unwrap();
        assert_eq!(n, 2);
        let text = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].split(',').count(), HEADER.len());
        // Best set is 90×10 to failure: e1RM 120, e5RM 102.857.
        assert_eq!(
            lines[1],
            "b,\"home, gym\",full_body,1,1,1,2026-01-01,FB 1,1,curl,Curl,,,,1,1,100,0,5,,,0,5,116.667,1,120,102.857,0,\"said \"\"easy\"\"\""
        );
        assert!(lines[2].contains(",120,102.857,0,"));
    }
}
