#[derive(Debug, Clone, Copy)]
pub enum PasswordStrengthScore {
    VeryWeak,
    Weak,
    StillWeak,
    Ok,
    Strong,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn password_strength(password: &str) -> PasswordStrengthScore {
    use accounts_base::client::password::Score;

    let score = accounts_base::client::password::password_strength(password);

    match score.score() {
        Score::One => PasswordStrengthScore::Weak,
        Score::Two => PasswordStrengthScore::StillWeak,
        Score::Three => PasswordStrengthScore::Ok,
        Score::Four => PasswordStrengthScore::Strong,
        _ => PasswordStrengthScore::VeryWeak,
    }
}

#[cfg(target_arch = "wasm32")]
pub fn password_strength(_password: &str) -> PasswordStrengthScore {
    PasswordStrengthScore::VeryWeak
}
