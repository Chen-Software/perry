#[cfg(test)]
mod tests {
    use super::super::verification::verify_image;

    #[tokio::test]
    async fn test_verification_cache_idempotence() {
        let img = "cgr.dev/chainguard/alpine-base";
        let res1 = verify_image(img).await;
        let res2 = verify_image(img).await;

        match (res1, res2) {
            (Ok(d1), Ok(d2)) => assert_eq!(d1, d2),
            (Err(e1), Err(e2)) => assert_eq!(e1.to_string(), e2.to_string()),
            _ => panic!("Non-idempotent result for image verification"),
        }
    }
}
