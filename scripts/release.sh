rm -rf release
mkdir release
cd release
cp ../target/default/debug/oasis-ssvm-runtime ./
cp ../target/default/debug/gateway ./
strip gateway
strip oasis-ssvm-runtime
tar zcvf oasis-ssvm-runtime.tgz gateway oasis-ssvm-runtime
echo SHA1:
sha1sum gateway oasis-ssvm-runtime | sed -E 's/[0-9a-f]{40}/`\0`/'
echo
cp ./oasis-ssvm-runtime.tgz ../
rm -rf release
