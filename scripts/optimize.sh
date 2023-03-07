docker run --rm -v "$(pwd)":/code \
	--mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
	--mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  --env RUST_BACKTRACE=1 \
	cosmwasm/workspace-optimizer-arm64:0.12.11
