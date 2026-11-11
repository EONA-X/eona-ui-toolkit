//! The eight series colours (`--eona-chart-series-1..8`) against the chart
//! surfaces: contrast of every mark and colour-vision separation of neighbours.
//! The thresholds are the dataviz skill's palette validator's (OKLab ΔE x100,
//! Machado-Oliveira-Fernandes severity 1.0).

use eona_ui_toolkit::TOKENS_CSS;

fn series(block: &str) -> Vec<[f64; 3]> {
    (1..=8)
        .map(|n| {
            let key = format!("--eona-chart-series-{n}: #");
            let at = block.find(&key).unwrap_or_else(|| panic!("series {n} missing")) + key.len();
            let hex = &block[at..at + 6];
            let c = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap() as f64 / 255.0;
            [c(0), c(2), c(4)]
        })
        .collect()
}

fn lin(c: f64) -> f64 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn luminance(c: [f64; 3]) -> f64 {
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

fn contrast(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

fn oklab(c: [f64; 3]) -> [f64; 3] {
    let [r, g, b] = [lin(c[0]), lin(c[1]), lin(c[2])];
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    [
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    ]
}

fn simulate(m: [[f64; 3]; 3], c: [f64; 3]) -> [f64; 3] {
    let v = [lin(c[0]), lin(c[1]), lin(c[2])];
    let out = |row: [f64; 3]| (row[0] * v[0] + row[1] * v[1] + row[2] * v[2]).clamp(0.0, 1.0);
    let enc = |x: f64| if x <= 0.0031308 { 12.92 * x } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 };
    [enc(out(m[0])), enc(out(m[1])), enc(out(m[2]))]
}

fn delta_e(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (x, y) = (oklab(a), oklab(b));
    100.0 * ((x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2) + (x[2] - y[2]).powi(2)).sqrt()
}

const PROTAN: [[f64; 3]; 3] = [[0.152286, 1.052583, -0.204868], [0.114503, 0.786281, 0.099216], [-0.003882, -0.048116, 1.051998]];
const DEUTAN: [[f64; 3]; 3] = [[0.367322, 0.860646, -0.227968], [0.280085, 0.672501, 0.047413], [-0.011820, 0.042940, 0.968881]];

fn check(name: &str, colours: &[[f64; 3]], surface: [f64; 3]) {
    for (i, c) in colours.iter().enumerate() {
        let k = contrast(*c, surface);
        assert!(k >= 3.0, "{name} series {} has {k:.2}:1 against the surface", i + 1);
    }
    for (i, w) in colours.windows(2).enumerate() {
        for (label, m) in [("protan", PROTAN), ("deutan", DEUTAN)] {
            let d = delta_e(simulate(m, w[0]), simulate(m, w[1]));
            assert!(d >= 8.0, "{name} series {}/{} under {label}: ΔE {d:.1} < 8", i + 1, i + 2);
        }
        let d = delta_e(w[0], w[1]);
        assert!(d >= 15.0, "{name} series {}/{} in normal vision: ΔE {d:.1} < 15", i + 1, i + 2);
    }
}

#[test]
fn light_series_are_distinguishable_and_clear_3_to_1_on_white() {
    let light = TOKENS_CSS.split("[data-theme=\"dark\"]").next().unwrap();
    check("light", &series(light), [1.0, 1.0, 1.0]);
}

#[test]
fn dark_series_are_distinguishable_and_clear_3_to_1_on_slate() {
    let dark = TOKENS_CSS.split("[data-theme=\"dark\"]").nth(1).expect("a dark block");
    check("dark", &series(dark), [0x24 as f64 / 255.0, 0x39 as f64 / 255.0, 0x55 as f64 / 255.0]);
}
