use tokio::process::Command;

pub(super) async fn system_lang() -> String {
    Command::new("defaults")
        .args(["read", "-g", "AppleLocale"])
        .output()
        .await
        .ok()
        .and_then(|output| {
            if output.status.success() {
                let locale = String::from_utf8_lossy(&output.stdout).trim().to_string();
                Some(locale.split('@').next().unwrap_or(&locale).to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "en_US".to_string())
}
