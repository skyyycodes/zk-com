//! A minimal lightwalletd client: the few `CompactTxStreamer` calls Z-Solvency
//! needs to read Ironwood chain data. Messages are written by hand from
//! lightwallet-protocol's `service.proto` and `compact_formats.proto` (only
//! the fields used here; prost skips the rest), so no protoc build step.

use anyhow::{anyhow, Context, Result};
use tonic::codegen::http::uri::PathAndQuery;
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};

#[derive(Clone, PartialEq, prost::Message)]
pub struct Empty {}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ChainSpec {}

#[derive(Clone, PartialEq, prost::Message)]
pub struct BlockId {
    #[prost(uint64, tag = "1")]
    pub height: u64,
    #[prost(bytes = "vec", tag = "2")]
    pub hash: Vec<u8>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct BlockRange {
    #[prost(message, optional, tag = "1")]
    pub start: Option<BlockId>,
    #[prost(message, optional, tag = "2")]
    pub end: Option<BlockId>,
    /// PoolType: IRONWOOD = 4.
    #[prost(int32, repeated, tag = "3")]
    pub pool_types: Vec<i32>,
}

pub const POOL_IRONWOOD: i32 = 4;

#[derive(Clone, PartialEq, prost::Message)]
pub struct ChainMetadata {
    #[prost(uint32, tag = "3")]
    pub ironwood_commitment_tree_size: u32,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct CompactAction {
    #[prost(bytes = "vec", tag = "1")]
    pub nullifier: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub cmx: Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub ephemeral_key: Vec<u8>,
    #[prost(bytes = "vec", tag = "4")]
    pub ciphertext: Vec<u8>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct CompactTx {
    #[prost(uint64, tag = "1")]
    pub index: u64,
    #[prost(bytes = "vec", tag = "2")]
    pub txid: Vec<u8>,
    #[prost(message, repeated, tag = "9")]
    pub ironwood_actions: Vec<CompactAction>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct CompactBlock {
    #[prost(uint64, tag = "2")]
    pub height: u64,
    #[prost(bytes = "vec", tag = "3")]
    pub hash: Vec<u8>,
    #[prost(bytes = "vec", tag = "4")]
    pub prev_hash: Vec<u8>,
    #[prost(uint32, tag = "5")]
    pub time: u32,
    #[prost(message, repeated, tag = "7")]
    pub vtx: Vec<CompactTx>,
    #[prost(message, optional, tag = "8")]
    pub chain_metadata: Option<ChainMetadata>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct TreeState {
    #[prost(string, tag = "1")]
    pub network: String,
    #[prost(uint64, tag = "2")]
    pub height: u64,
    #[prost(string, tag = "3")]
    pub hash: String,
    #[prost(string, tag = "7")]
    pub ironwood_tree: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct LightdInfo {
    #[prost(string, tag = "1")]
    pub version: String,
    #[prost(string, tag = "2")]
    pub vendor: String,
    #[prost(string, tag = "4")]
    pub chain_name: String,
    #[prost(string, tag = "6")]
    pub consensus_branch_id: String,
    #[prost(uint64, tag = "7")]
    pub block_height: u64,
    #[prost(uint64, tag = "12")]
    pub estimated_height: u64,
    #[prost(string, tag = "13")]
    pub zcashd_build: String,
    #[prost(string, tag = "14")]
    pub zcashd_subversion: String,
    #[prost(string, tag = "16")]
    pub upgrade_name: String,
    #[prost(string, tag = "18")]
    pub lightwallet_protocol_version: String,
}

const SERVICE: &str = "/cash.z.wallet.sdk.rpc.CompactTxStreamer/";

/// A blocking client over one gRPC channel.
pub struct Lightwalletd {
    rt: tokio::runtime::Runtime,
    grpc: tonic::client::Grpc<Channel>,
}

fn path(method: &str) -> PathAndQuery {
    PathAndQuery::try_from(format!("{SERVICE}{method}")).unwrap()
}

impl Lightwalletd {
    pub fn connect(url: &str) -> Result<Self> {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        let mut ep = Endpoint::from_shared(url.to_string())?.timeout(std::time::Duration::from_secs(120));
        if url.starts_with("https://") {
            ep = ep.tls_config(ClientTlsConfig::new().with_webpki_roots())?;
        }
        let ch = rt.block_on(ep.connect()).with_context(|| format!("connecting to {url}"))?;
        Ok(Lightwalletd { rt, grpc: tonic::client::Grpc::new(ch) })
    }

    fn unary<Q, R>(&mut self, method: &str, req: Q) -> Result<R>
    where
        Q: prost::Message + Send + Sync + 'static,
        R: prost::Message + Default + Send + Sync + 'static,
    {
        let g = &mut self.grpc;
        self.rt.block_on(async {
            g.ready().await.map_err(|e| anyhow!("{e}"))?;
            let r = g
                .unary(tonic::Request::new(req), path(method), tonic_prost::ProstCodec::<Q, R>::default())
                .await
                .map_err(|s| anyhow!("{method}: {}", s.message()))?;
            Ok(r.into_inner())
        })
    }

    pub fn info(&mut self) -> Result<LightdInfo> {
        self.unary("GetLightdInfo", Empty {})
    }

    pub fn tip(&mut self) -> Result<u64> {
        Ok(self.unary::<_, BlockId>("GetLatestBlock", ChainSpec {})?.height)
    }

    pub fn tree_state(&mut self, height: u64) -> Result<TreeState> {
        self.unary("GetTreeState", BlockId { height, hash: vec![] })
    }

    /// Stream compact blocks `start..=end` (Ironwood data only) into `f`.
    pub fn blocks(&mut self, start: u64, end: u64, mut f: impl FnMut(CompactBlock) -> Result<()>) -> Result<()> {
        let g = &mut self.grpc;
        self.rt.block_on(async {
            g.ready().await.map_err(|e| anyhow!("{e}"))?;
            let req = BlockRange {
                start: Some(BlockId { height: start, hash: vec![] }),
                end: Some(BlockId { height: end, hash: vec![] }),
                pool_types: vec![POOL_IRONWOOD],
            };
            let mut s = g
                .server_streaming(tonic::Request::new(req), path("GetBlockRange"), tonic_prost::ProstCodec::<BlockRange, CompactBlock>::default())
                .await
                .map_err(|s| anyhow!("GetBlockRange: {}", s.message()))?
                .into_inner();
            while let Some(b) = s.message().await.map_err(|s| anyhow!("GetBlockRange: {}", s.message()))? {
                f(b)?;
            }
            Ok(())
        })
    }
}

/// Block and transaction ids travel in internal byte order; explorers show them reversed.
pub fn display_hash(internal: &[u8]) -> String {
    let mut v = internal.to_vec();
    v.reverse();
    hex::encode(v)
}
