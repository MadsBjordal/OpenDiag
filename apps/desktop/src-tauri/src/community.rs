//! Community data (opt-in): sends a shared result the user has seen and
//! confirmed, deletes it again on request, and downloads the combined
//! summaries. Talks only to the OpenDiag data service (services/feedback).

use crate::update::agent;
use serde_json::Value;

/// The data service. A build can point elsewhere with OPENDIAG_DATA_URL.
fn base() -> &'static str {
    option_env!("OPENDIAG_DATA_URL").unwrap_or("https://opendiag-data.stencilshot.com")
}

fn err(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(code, r) => {
            let msg = r
                .into_json::<Value>()
                .ok()
                .and_then(|v| v["error"].as_str().map(str::to_string))
                .unwrap_or_default();
            format!(
                "The OpenDiag data service refused it ({code}{}).",
                if msg.is_empty() {
                    String::new()
                } else {
                    format!(": {msg}")
                }
            )
        }
        e => format!("Could not reach the OpenDiag data service: {e}"),
    }
}

/// Sends one share (the JSON the engine built and the user saw). Returns `{id, delete_token}`.
pub fn send(share: &Value) -> Result<Value, String> {
    agent()
        .post(&format!("{}/v1/shares", base()))
        .send_json(share)
        .map_err(err)?
        .into_json()
        .map_err(|e| e.to_string())
}

pub fn delete(id: &str, token: &str) -> Result<(), String> {
    if !id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return Err("bad id".into());
    }
    match agent()
        .delete(&format!("{}/v1/shares/{id}", base()))
        .set("Authorization", &format!("Bearer {token}"))
        .call()
    {
        Ok(_) | Err(ureq::Error::Status(404, _)) => Ok(()),
        Err(e) => Err(err(e)),
    }
}

/// The combined compatibility snapshot (as a JSON string, for the engine's import) and the fault code counts.
pub fn fetch() -> Result<Value, String> {
    let snapshot = agent()
        .get(&format!("{}/v1/snapshot", base()))
        .call()
        .map_err(err)?
        .into_string()
        .map_err(|e| e.to_string())?;
    let codes: Value = agent()
        .get(&format!("{}/v1/codes", base()))
        .call()
        .map_err(err)?
        .into_json()
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "snapshot": snapshot, "codes": codes }))
}

#[cfg(test)]
mod tests {
    use opendiag_engine::Engine;
    use opendiag_sim::{profiles, SimAdapter, SimSettings};
    use opendiag_store::Store;

    /// Round trip against a running data service: build with
    /// OPENDIAG_DATA_URL=http://127.0.0.1:8799 and start `npm run dev` in
    /// services/feedback, then `cargo test -p opendiag-desktop -- --ignored`.
    #[test]
    #[ignore]
    fn share_round_trip_with_the_data_service() {
        let mut e = Engine::new(Store::open_in_memory().unwrap(), "windows");
        let (link, ctl) = SimAdapter::new(
            profiles::car(),
            SimSettings {
                latency_ms: 0,
                ..Default::default()
            },
        );
        e.connect(Box::new(link), Some(ctl)).unwrap();
        let r = e.scan(&mut |_| {}).unwrap();
        let id: i64 = r.id.parse().unwrap();
        let mut real = e.report(id).unwrap();
        real.simulated = false; // the service never sees simulator data in practice
        e.store()
            .update_report(id, &serde_json::to_value(&real).unwrap())
            .unwrap();
        let mut share = serde_json::to_value(e.share_preview(id, &[]).unwrap()).unwrap();
        // The simulator's VIN has no known manufacturer; users name the make in My Garage.
        share["vehicle"]["make"] = "Testmake".into();
        let first = super::send(&share).unwrap();
        let second = super::send(&share).unwrap();
        let got = super::fetch().unwrap();
        let n = e
            .import_community(got["snapshot"].as_str().unwrap(), &got["codes"])
            .unwrap();
        assert!(n > 0, "{got}");
        assert!(!e.community_codes("Testmake", None, None).is_empty());
        for s in [first, second] {
            super::delete(
                s["id"].as_str().unwrap(),
                s["delete_token"].as_str().unwrap(),
            )
            .unwrap();
        }
    }
}
