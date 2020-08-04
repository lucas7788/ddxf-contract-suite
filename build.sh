#cd ./contracts/dtoken
#cd ./contracts/marketplace
cd ./contracts/openkg
#cd ./contracts/data_id
#cd ./contracts/split_policy
#cd ./contracts/accountant

cargo build --release --target=wasm32-unknown-unknown

cd ../../
mkdir -p output
for wasm in $(ls ./target/wasm32-unknown-unknown/release/*.wasm); do
	ontio-wasm-build $wasm output/$(basename $wasm)
	ontio-wasm-build $wasm output/$(basename $wasm).str
done
