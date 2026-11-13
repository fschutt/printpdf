update:
	cargo update --verbose

features:
	cargo features

checkup:
	cargo clippy --workspace --all-targets --all-features -- -D warnings
	cargo check --workspace --all-targets --all-features
	cargo shear
	cargo machete
	cargo audit

fix:
	cargo fix

fix-all:
	cargo fix --all
	cargo clippy --workspace --all-targets --all-features --fix

test:
	cargo test --workspace --all-targets --all-features

.PHONY: update features checkup fix fix-all test

# -- ⚝  by Dave -- in NeoVim ⚝  --
