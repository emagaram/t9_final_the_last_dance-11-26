use aws_config::{meta::region::RegionProviderChain, BehaviorVersion};
use aws_sdk_s3::{primitives::ByteStream, Client};
use std::fs;
use std::path::Path;

pub async fn save_to_s3(local_folder: &str) -> Result<(), Box<dyn std::error::Error>> {
    let local_folder = Path::new(local_folder);
    if !local_folder.exists() {
        println!(
            "Local folder {:?} does not exist. Nothing to do.",
            local_folder
        );
        return Ok(());
    }

    let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(region_provider)
        .load()
        .await;
    let client = Client::new(&config);
    let bucket = "t9final";

    for entry in fs::read_dir(local_folder)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = match path.file_name() {
            Some(name) => name.to_string_lossy().into_owned(),
            None => continue,
        };

        // TODO, this needs to work for scorecasts and other nested files
        let key = format!("proj/data/{}", file_name);

        if path.is_file() {
            println!("Uploading file: {:?} to s3://{}/{}", &path, bucket, &key);
            let body = ByteStream::from_path(&path).await?;
            client
                .put_object()
                .bucket(bucket)
                .key(&key)
                .body(body)
                .send()
                .await?;
            println!("Successfully uploaded. Removing local file: {:?}", &path);
            fs::remove_file(&path)?;
        } else if path.is_dir() {
            println!("Found directory: {:?}. Uploading its contents.", &path);
            fn upload_directory_contents<'a>(
                client: &'a Client,
                bucket: &'a str,
                dir_path: &'a Path,
                s3_key_prefix: &'a str,
            ) -> std::pin::Pin<
                Box<
                    dyn std::future::Future<Output = Result<(), Box<dyn std::error::Error>>>
                        + Send
                        + 'a,
                >,
            > {
                Box::pin(async move {
                    for entry in fs::read_dir(dir_path)? {
                        let entry = entry?;
                        let path = entry.path();
                        let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                        let key = format!("{}/{}", s3_key_prefix, file_name);

                        if path.is_file() {
                            println!("Uploading file: {:?} to s3://{}/{}", &path, bucket, &key);
                            let body = ByteStream::from_path(&path).await?;
                            client
                                .put_object()
                                .bucket(bucket.to_string())
                                .key(&key)
                                .body(body)
                                .send()
                                .await?;
                            println!("Successfully uploaded. Removing local file: {:?}", &path);
                            fs::remove_file(&path)?;
                        } else if path.is_dir() {
                            upload_directory_contents(client, bucket, &path, &key).await?;
                        }
                    }
                    // After uploading contents, remove the now-empty directory
                    println!("Removing local directory: {:?}", dir_path);
                    fs::remove_dir(dir_path)?;
                    Ok(())
                })
            }

            upload_directory_contents(&client, bucket, &path, &key).await?;
        }
    }

    Ok(())
}
