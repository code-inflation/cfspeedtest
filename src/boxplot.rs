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
