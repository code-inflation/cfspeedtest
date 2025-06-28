use log;
use std::fmt::Write;

const PLOT_WIDTH: usize = 80;

fn generate_axis_labels(minima: f64, maxima: f64) -> String {
    let mut labels = String::new();
    write!(labels, "{:<10.2}", minima).unwrap();
    write!(
        labels,
        "{:^width$.2}",
        (minima + maxima) / 2.0,
        width = PLOT_WIDTH - 20
    )
    .unwrap();
    write!(labels, "{:>10.2}", maxima).unwrap();
    labels
}

pub(crate) fn render_plot(minima: f64, q1: f64, median: f64, q3: f64, maxima: f64) -> String {
    let value_range = maxima - minima;
    let quartile_0 = q1 - minima;
    let quartile_1 = median - q1;
    let quartile_2 = q3 - median;
    let quartile_3 = maxima - q3;

    let scale_factor = PLOT_WIDTH as f64 / value_range;

    let mut plot = String::with_capacity(PLOT_WIDTH);
    plot.push('|');
    plot.push_str("-".repeat((quartile_0 * scale_factor) as usize).as_str());
    plot.push_str("=".repeat((quartile_1 * scale_factor) as usize).as_str());
    plot.push(':');
    plot.push_str("=".repeat((quartile_2 * scale_factor) as usize).as_str());
    plot.push_str("-".repeat((quartile_3 * scale_factor) as usize).as_str());
    plot.push('|');

    let axis_labels = generate_axis_labels(minima, maxima);
    plot.push('\n');
    plot.push_str(&axis_labels);

    log::debug!("fn input: {minima}, {q1}, {median}, {q3}, {maxima}");
    log::debug!("quartiles: {quartile_0}, {quartile_1}, {quartile_2}, {quartile_3}");
    log::debug!("value range: {value_range}");
    log::debug!("len of the plot: {}", plot.len());

    plot
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_axis_labels() {
        let labels = generate_axis_labels(0.0, 100.0);
        assert!(labels.starts_with("0.00"));
        assert!(labels.ends_with("100.00"));
        assert!(labels.contains("50.00"));
        assert_eq!(labels.len(), PLOT_WIDTH);
    }

    #[test]
    fn test_generate_axis_labels_negative() {
        let labels = generate_axis_labels(-50.0, 50.0);
        assert!(labels.starts_with("-50.00"));
        assert!(labels.ends_with("50.00"));
        assert!(labels.contains("0.00"));
    }

    #[test]
    fn test_render_plot_basic() {
        let plot = render_plot(0.0, 25.0, 50.0, 75.0, 100.0);

        // Should contain boxplot characters
        assert!(plot.contains('|'));
        assert!(plot.contains('-'));
        assert!(plot.contains('='));
        assert!(plot.contains(':'));

        // Should contain axis labels
        assert!(plot.contains("0.00"));
        assert!(plot.contains("100.00"));
        assert!(plot.contains("50.00"));

        // Should have newline separating plot from labels
        assert!(plot.contains('\n'));
    }

    #[test]
    fn test_render_plot_same_values() {
        let plot = render_plot(50.0, 50.0, 50.0, 50.0, 50.0);

        // When all values are the same, should still render
        assert!(plot.contains('|'));
        assert!(plot.contains(':'));
        assert!(plot.contains("50.00"));
    }

    #[test]
    fn test_render_plot_structure() {
        let plot = render_plot(10.0, 30.0, 50.0, 70.0, 90.0);
        let lines: Vec<&str> = plot.split('\n').collect();

        // Should have exactly 2 lines: plot and axis labels
        assert_eq!(lines.len(), 2);

        // First line should be the boxplot
        assert!(lines[0].starts_with('|'));
        assert!(lines[0].ends_with('|'));

        // Second line should be the axis labels
        assert!(lines[1].contains("10.00"));
        assert!(lines[1].contains("90.00"));
    }

    #[test]
    fn test_render_plot_quartile_ordering() {
        let plot = render_plot(0.0, 20.0, 50.0, 80.0, 100.0);

        // Find the positions of key characters
        let colon_pos = plot.find(':').unwrap();
        let first_pipe = plot.find('|').unwrap();
        let last_pipe = plot.rfind('|').unwrap();

        // Colon (median) should be between the pipes
        assert!(colon_pos > first_pipe);
        assert!(colon_pos < last_pipe);
    }
}
