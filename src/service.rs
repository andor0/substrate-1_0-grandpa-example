//! Service and ServiceFactory implementation. Specialized wrapper over Substrate service.

#![warn(unused_extern_crates)]

use std::sync::Arc;
use std::time::Duration;

use log::info;
use transaction_pool::{self, txpool::{Pool as TransactionPool}};
use substrate_1_0_grandpa_example_runtime::{self, GenesisConfig, opaque::Block, RuntimeApi};
use substrate_service::{
	FactoryFullConfiguration, LightComponents, FullComponents, FullBackend,
	FullClient, LightClient, LightBackend, FullExecutor, LightExecutor, TaskExecutor,
};
use grandpa;
use consensus::{import_queue, start_aura, AuraImportQueue, SlotDuration, NothingExtra};
use substrate_client as client;
use primitives::{Pair as PairT, ed25519};
use inherents::InherentDataProviders;
use network::construct_simple_protocol;
use substrate_executor::native_executor_instance;
use substrate_service::construct_service_factory;

pub use substrate_executor::NativeExecutor;
// Our native executor instance.
native_executor_instance!(
	pub Executor,
	substrate_1_0_grandpa_example_runtime::api::dispatch,
	substrate_1_0_grandpa_example_runtime::native_version,
	include_bytes!("../runtime/wasm/target/wasm32-unknown-unknown/release/substrate_1_0_grandpa_example_runtime_wasm.compact.wasm")
);

construct_simple_protocol! {
	/// Demo protocol attachment for substrate.
	pub struct NodeProtocol where Block = Block { }
}

/// Node specific configuration
pub struct NodeConfig<F: substrate_service::ServiceFactory> {
	/// grandpa connection to import block
	// FIXME #1134 rather than putting this on the config, let's have an actual intermediate setup state
	pub grandpa_import_setup: Option<(Arc<grandpa::BlockImportForService<F>>, grandpa::LinkHalfForService<F>)>,
	inherent_data_providers: InherentDataProviders,
}

impl<F> Default for NodeConfig<F> where F: substrate_service::ServiceFactory {
	fn default() -> NodeConfig<F> {
		NodeConfig {
			grandpa_import_setup: None,
			inherent_data_providers: InherentDataProviders::new(),
		}
	}
}

construct_service_factory! {
	struct Factory {
		Block = Block,
		RuntimeApi = RuntimeApi,
		NetworkProtocol = NodeProtocol { |config| Ok(NodeProtocol::new()) },
		RuntimeDispatch = Executor,
		FullTransactionPoolApi = transaction_pool::ChainApi<client::Client<FullBackend<Self>, FullExecutor<Self>, Block, RuntimeApi>, Block>
			{ |config, client| Ok(TransactionPool::new(config, transaction_pool::ChainApi::new(client))) },
		LightTransactionPoolApi = transaction_pool::ChainApi<client::Client<LightBackend<Self>, LightExecutor<Self>, Block, RuntimeApi>, Block>
			{ |config, client| Ok(TransactionPool::new(config, transaction_pool::ChainApi::new(client))) },
		Genesis = GenesisConfig,
		Configuration = NodeConfig<Self>,
		FullService = FullComponents<Self>
			{ |config: FactoryFullConfiguration<Self>, executor: TaskExecutor|
				FullComponents::<Factory>::new(config, executor)
			},
		AuthoritySetup = {
			|mut service: Self::FullService, executor: TaskExecutor, local_key: Option<Arc<ed25519::Pair>>| {
				let (block_import, link_half) = service.config.custom.grandpa_import_setup.take()
					.expect("Link Half and Block Import are present for Full Services or setup failed before. qed");

				if let Some(ref key) = local_key {
					info!("Using authority key {}", key.public());
					let proposer = Arc::new(substrate_basic_authorship::ProposerFactory {
						client: service.client(),
						transaction_pool: service.transaction_pool(),
						inherents_pool: service.inherents_pool(),
					});

					let client = service.client();
					executor.spawn(start_aura(
						SlotDuration::get_or_compute(&*client)?,
						key.clone(),
						client,
						block_import.clone(),
						proposer,
						service.network(),
						service.on_exit(),
						service.config.custom.inherent_data_providers.clone(),
						service.config.force_authoring,
					)?);

					info!("Running Grandpa session as Authority {}", key.public());
				}

				let local_key = if service.config.disable_grandpa {
					None
				} else {
					local_key
				};

				executor.spawn(grandpa::run_grandpa(
					grandpa::Config {
						local_key,
						// FIXME #1578 make this available through chainspec
						gossip_duration: Duration::from_millis(333),
						justification_period: 4096,
						name: Some(service.config.name.clone())
					},
					link_half,
					grandpa::NetworkBridge::new(service.network()),
					service.config.custom.inherent_data_providers.clone(),
					service.on_exit(),
				)?);

				Ok(service)
			}
		},
	LightService = LightComponents<Self>
			{ |config, executor| <LightComponents<Factory>>::new(config, executor) },
		FullImportQueue = AuraImportQueue<Self::Block>
			{ |config: &mut FactoryFullConfiguration<Self> , client: Arc<FullClient<Self>>| {
				let slot_duration = SlotDuration::get_or_compute(&*client)?;
				let (block_import, link_half) =
					grandpa::block_import::<_, _, _, RuntimeApi, FullClient<Self>>(
						client.clone(), client.clone()
					)?;
				let block_import = Arc::new(block_import);
				let justification_import = block_import.clone();

				config.custom.grandpa_import_setup = Some((block_import.clone(), link_half));

				import_queue::<_, _, _, ed25519::Pair>(
					slot_duration,
					block_import,
					Some(justification_import),
					client,
					NothingExtra,
					config.custom.inherent_data_providers.clone(),
				).map_err(Into::into)
			}},
		LightImportQueue = AuraImportQueue<Self::Block>
			{ |config: &FactoryFullConfiguration<Self>, client: Arc<LightClient<Self>>| {
				import_queue::<_, _, _, ed25519::Pair>(
					SlotDuration::get_or_compute(&*client)?,
					client.clone(),
					None,
					client,
					NothingExtra,
					config.custom.inherent_data_providers.clone(),
				).map_err(Into::into)
			}
		},
	}
}


#[cfg(test)]
mod tests {
	#[cfg(feature = "rhd")]
	fn test_sync() {
		use {service_test, Factory};
		use client::{ImportBlock, BlockOrigin};

		let alice: Arc<ed25519::Pair> = Arc::new(Keyring::Alice.into());
		let bob: Arc<ed25519::Pair> = Arc::new(Keyring::Bob.into());
		let validators = vec![alice.public().0.into(), bob.public().0.into()];
		let keys: Vec<&ed25519::Pair> = vec![&*alice, &*bob];
		let dummy_runtime = ::tokio::runtime::Runtime::new().unwrap();
		let block_factory = |service: &<Factory as service::ServiceFactory>::FullService| {
			let block_id = BlockId::number(service.client().info().unwrap().chain.best_number);
			let parent_header = service.client().header(&block_id).unwrap().unwrap();
			let consensus_net = ConsensusNetwork::new(service.network(), service.client().clone());
			let proposer_factory = consensus::ProposerFactory {
				client: service.client().clone(),
				transaction_pool: service.transaction_pool().clone(),
				network: consensus_net,
				force_delay: 0,
				handle: dummy_runtime.executor(),
			};
			let (proposer, _, _) = proposer_factory.init(&parent_header, &validators, alice.clone()).unwrap();
			let block = proposer.propose().expect("Error making test block");
			ImportBlock {
				origin: BlockOrigin::File,
				justification: Vec::new(),
				internal_justification: Vec::new(),
				finalized: true,
				body: Some(block.extrinsics),
				header: block.header,
				auxiliary: Vec::new(),
			}
		};
		let extrinsic_factory = |service: &<Factory as service::ServiceFactory>::FullService| {
			let payload = (0, Call::Balances(BalancesCall::transfer(RawAddress::Id(bob.public().0.into()), 69.into())), Era::immortal(), service.client().genesis_hash());
			let signature = alice.sign(&payload.encode()).into();
			let id = alice.public().0.into();
			let xt = UncheckedExtrinsic {
				signature: Some((RawAddress::Id(id), signature, payload.0, Era::immortal())),
				function: payload.1,
			}.encode();
			let v: Vec<u8> = Decode::decode(&mut xt.as_slice()).unwrap();
			OpaqueExtrinsic(v)
		};
		service_test::sync::<Factory, _, _>(chain_spec::integration_test_config(), block_factory, extrinsic_factory);
	}

}
