use log;
const PLOT_WIDTH: usize = 40;

pub(crate) fn render_plot(minima: f64, q1: f64, median: f64, q3: f64, maxima: f64) -> String {
    // TODO print axis labels
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

    log::debug!("fn input: {minima}, {q1}, {median}, {q3}, {maxima}");
    log::debug!("quartiles: {quartile_0}, {quartile_1}, {quartile_2}, {quartile_3}");
    log::debug!("value range: {value_range}");
    log::debug!("len of the plot: {}", plot.len());

    plot
}
