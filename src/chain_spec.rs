use primitives::{ed25519, sr25519, Pair};
use substrate_1_0_grandpa_example_runtime::{
	AccountId, Perbill, Permill, GenesisConfig, ConsensusConfig, TimestampConfig, BalancesConfig,
	SudoConfig, IndicesConfig, SessionConfig, StakingConfig, DemocracyConfig,
	CouncilVotingConfig, TreasuryConfig, GrandpaConfig
};
use substrate_service;

use ed25519::Public as AuthorityId;

// Note this is the URL for the telemetry server
//const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = substrate_service::ChainSpec<GenesisConfig>;

/// The chain specification option. This is expected to come in from the CLI and
/// is little more than one of a number of alternatives which can easily be converted
/// from a string (`--chain=...`) into a `ChainSpec`.
#[derive(Clone, Debug)]
pub enum Alternative {
	/// Whatever the current runtime is, with just Alice as an auth.
	Development,
	/// Whatever the current runtime is, with simple Alice/Bob auths.
	LocalTestnet,
}

fn authority_key(s: &str) -> AuthorityId {
	ed25519::Pair::from_string(&format!("//{}", s), None)
		.expect("static values are valid; qed")
		.public()
}

fn account_key(s: &str) -> AccountId {
	sr25519::Pair::from_string(&format!("//{}", s), None)
		.expect("static values are valid; qed")
		.public()
}

impl Alternative {
	/// Get an actual chain config from one of the alternatives.
	pub(crate) fn load(self) -> Result<ChainSpec, String> {
		Ok(match self {
			Alternative::Development => ChainSpec::from_genesis(
				"Development",
				"dev",
				|| testnet_genesis(vec![
					authority_key("Alice")
				], vec![
					account_key("Alice")
				],
					account_key("Alice")
				),
				vec![],
				None,
				None,
				None,
				None
			),
			Alternative::LocalTestnet => ChainSpec::from_genesis(
				"Local Testnet",
				"local_testnet",
				|| testnet_genesis(vec![
					authority_key("Alice"),
					authority_key("Bob"),
				], vec![
					account_key("Alice"),
					account_key("Bob"),
					account_key("Charlie"),
					account_key("Dave"),
					account_key("Eve"),
					account_key("Ferdie"),
				],
					account_key("Alice"),
				),
				vec![],
				None,
				None,
				None,
				None
			),
		})
	}

	pub(crate) fn from(s: &str) -> Option<Self> {
		match s {
			"dev" => Some(Alternative::Development),
			"" | "local" => Some(Alternative::LocalTestnet),
			_ => None,
		}
	}
}

fn testnet_genesis(initial_authorities: Vec<AuthorityId>, endowed_accounts: Vec<AccountId>, root_key: AccountId) -> GenesisConfig {
	GenesisConfig {
		consensus: Some(ConsensusConfig {
			code: include_bytes!("../runtime/wasm/target/wasm32-unknown-unknown/release/substrate_1_0_grandpa_example_runtime_wasm.compact.wasm").to_vec(),
			authorities: initial_authorities.clone(),
		}),
		system: None,
		timestamp: Some(TimestampConfig {
			minimum_period: 5, // 10 second block time.
		}),
		indices: Some(IndicesConfig {
			ids: endowed_accounts.clone(),
		}),
		balances: Some(BalancesConfig {
			transaction_base_fee: 1,
			transaction_byte_fee: 0,
			existential_deposit: 500,
			transfer_fee: 0,
			creation_fee: 0,
			balances: endowed_accounts.iter().cloned().map(|k|(k, 1 << 60)).collect(),
			vesting: vec![],
		}),
		session: Some(SessionConfig {
			validators: vec![account_key("Alice Controller"), account_key("Bob Controller")],
			keys: vec![account_key("Alice Controller"), account_key("Bob Controller")].iter().cloned().zip(initial_authorities.clone()).collect(),
			session_length: 6
		}),
		staking: Some(StakingConfig {
			validator_count: 5,
			minimum_validator_count: 1,
			sessions_per_era: 5,
			session_reward: Perbill::from_millionths(10_000),
			offline_slash: Perbill::from_percent(50_000),
			offline_slash_grace: 3,
			bonding_duration: 30,
			invulnerables: vec![],
			stakers: vec![],
			current_era: 0,
			current_session_reward: 10,
		}),
		democracy: Some(DemocracyConfig {
			launch_period: 1440,
			minimum_deposit: 10_000,
			public_delay: 5,
			max_lock_periods: 60,
			voting_period: 144,
		}),
		council_voting: Some(CouncilVotingConfig {
			cooloff_period: 360,
			voting_period: 60,
			enact_delay_period: 5,
		}),
		treasury: Some(TreasuryConfig {
			proposal_bond: Permill::from_millionths(50_000),
			proposal_bond_minimum: 1_000_000,
			spend_period: 360,
			burn: Permill::from_millionths(100_000),
		}),
		grandpa: Some(GrandpaConfig {
			authorities: initial_authorities.iter().cloned().map(|x| (x, 1)).collect()
		}),
		sudo: Some(SudoConfig {
			key: root_key,
		}),
	}
}
