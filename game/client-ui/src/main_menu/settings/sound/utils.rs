pub fn db_to_ratio(v: f64) -> f64 {
    (10_f64).powf(v / 20.0)
}
pub fn ratio_to_db(v: f64) -> f64 {
    v.log10() * 20.0
}
