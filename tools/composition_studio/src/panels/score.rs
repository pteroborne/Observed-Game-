//! Score panel readout and A/B comparison for `composition_studio`.

use observed_facility::hex_wfc::score::LayoutScore;

/// Format layout score component and delta vs baseline score.
pub fn format_score_breakdown(score: &LayoutScore, baseline: Option<&LayoutScore>) -> String {
    let delta_str = |val: f64, base_val: Option<f64>| -> String {
        match base_val {
            Some(bv) => {
                let diff = val - bv;
                if diff > 0.0001 {
                    format!(" (+{diff:.3})")
                } else if diff < -0.0001 {
                    format!(" ({diff:.3})")
                } else {
                    " (=)".to_string()
                }
            }
            None => String::new(),
        }
    };

    let base_total = baseline.map(|b| b.total);
    let base_conn = baseline.map(|b| b.connectivity);
    let base_elev = baseline.map(|b| b.elevation);
    let base_whol = baseline.map(|b| b.room_wholeness);
    let base_vari = baseline.map(|b| b.archetype_variety);
    let base_rhy = baseline.map(|b| b.junction_rhythm);

    format!(
        "SCORE BREAKDOWN (A/B vs Baseline):\n\
         Total Score:        {:.3}{}\n\
         - Connectivity:     {:.3}{}\n\
         - Elevation:        {:.3}{}\n\
         - Room Wholeness:   {:.3}{}\n\
         - Archetype Variety: {:.3}{}\n\
         - Junction Rhythm:  {:.3}{}",
        score.total,
        delta_str(score.total, base_total),
        score.connectivity,
        delta_str(score.connectivity, base_conn),
        score.elevation,
        delta_str(score.elevation, base_elev),
        score.room_wholeness,
        delta_str(score.room_wholeness, base_whol),
        score.archetype_variety,
        delta_str(score.archetype_variety, base_vari),
        score.junction_rhythm,
        delta_str(score.junction_rhythm, base_rhy),
    )
}
