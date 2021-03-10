
fn main() {
    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .compile(
            &[
                "proto/tendermint/version/types.proto",
                "proto/tendermint/crypto/proof.proto",
                "proto/tendermint/types/types.proto",
                "proto/tendermint/abci/types.proto",
                "proto/cosmos/base/abci/v1beta1/abci.proto",
                "proto/cosmos/base/query/v1beta1/pagination.proto",
                "proto/cosmos/tx/v1beta1/service.proto",
                "proto/cosmos/tx/signing/v1beta1/signing.proto",
                "proto/cosmos/crypto/multisig/v1beta1/multisig.proto",
                "proto/cosmos/base/v1beta1/coin.proto",
                "proto/cosmos/tx/v1beta1/tx.proto",
                "proto/ibc/lightclients/wasm/v1/wasm.proto",
                "proto/ibc/core/client/v1/tx.proto",
                "proto/cosmos/auth/v1beta1/auth.proto",
                "proto/cosmos/auth/v1beta1/query.proto"
            ],
            &["proto/"],
        ).unwrap();
}
