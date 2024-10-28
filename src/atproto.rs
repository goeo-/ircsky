use anyhow::Result;

pub async fn get_did_and_auth_endpoint(handle: &str) -> Result<(String, String)> {
    let did = resolve_handle(handle).await?;
    let pds = get_pds(&did).await?;
    let url = format!("{}/.well-known/oauth-protected-resource", pds);

    #[derive(serde::Deserialize)]
    struct ProtectedResource {
        authorization_servers: Vec<String>,
    }

    let auth_endpoint: ProtectedResource = reqwest::get(&url).await?.json().await?;
    Ok((
        did,
        auth_endpoint
            .authorization_servers
            .first()
            .cloned()
            .ok_or(anyhow::anyhow!("auth endpoint not found"))?,
    ))
}

pub async fn get_pds(did: &str) -> Result<String> {
    let did_doc = get_did_doc(did).await?;
    Ok(did_doc
        .service
        .iter()
        .find(|s| s.id == "#atproto_pds" && s.r#type == "AtprotoPersonalDataServer")
        .ok_or(anyhow::anyhow!("pds not found"))?
        .service_endpoint
        .clone())
}

pub async fn resolve_handle(handle: &str) -> Result<String> {
    let url = format!(
        "https://public.api.bsky.app/xrpc/com.atproto.identity.resolveHandle?handle={}",
        handle
    );

    #[derive(serde::Deserialize)]
    struct HandleResolution {
        did: String,
    }

    let did_doc: HandleResolution = reqwest::get(&url).await?.json().await?;
    Ok(did_doc.did)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DidDoc {
    pub also_known_as: Vec<String>,
    pub service: Vec<Service>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    pub id: String,
    pub r#type: String,
    pub service_endpoint: String,
}

pub async fn get_did_doc(did: &str) -> Result<DidDoc> {
    let url = match &did[..8] {
        "did:plc:" => format!("https://plc.directory/{did}"),
        "did:web:" => format!("https://{}/.well-known/did.json", &did[8..]),
        _ => anyhow::bail!("invalid did"),
    };

    Ok(reqwest::get(&url).await?.json().await?)
}
