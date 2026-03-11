.PHONY: publish-dry-run publish \
	publish-primitivo-macro-dry-run publish-primitivo-airdrop-merkle-dry-run \
	publish-primitivo-token-converter-dry-run publish-primitivo-vesting-crate-dry-run \
	publish-primitivo-vault-dry-run publish-primitivo-macro publish-primitivo-airdrop-merkle \
	publish-primitivo-token-converter publish-primitivo-vesting-crate publish-primitivo-vault

publish-dry-run: \
	publish-primitivo-macro-dry-run \
	publish-primitivo-airdrop-merkle-dry-run \
	publish-primitivo-token-converter-dry-run \
	publish-primitivo-vesting-crate-dry-run \
	publish-primitivo-vault-dry-run

publish: \
	publish-primitivo-macro \
	publish-primitivo-airdrop-merkle \
	publish-primitivo-token-converter \
	publish-primitivo-vesting-crate \
	publish-primitivo-vault

publish-primitivo-macro-dry-run:
	cargo package -p primitivo-macro

publish-primitivo-airdrop-merkle-dry-run:
	cargo package -p primitivo-airdrop-merkle

publish-primitivo-token-converter-dry-run:
	cargo package -p primitivo-token-converter

publish-primitivo-vesting-crate-dry-run:
	cargo package -p primitivo-vesting-crate

publish-primitivo-vault-dry-run:
	cargo package -p primitivo-vault

publish-primitivo-macro:
	cargo publish -p primitivo-macro

publish-primitivo-airdrop-merkle:
	cargo publish -p primitivo-airdrop-merkle

publish-primitivo-token-converter:
	cargo publish -p primitivo-token-converter

publish-primitivo-vesting-crate:
	cargo publish -p primitivo-vesting-crate

publish-primitivo-vault:
	cargo publish -p primitivo-vault
