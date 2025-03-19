TMPDIR=$(mktemp -d)
CURDIR=$(pwd)
CARGO=$(which cargo)
cd "$TMPDIR"
cp -r "$CURDIR" .
cd $(basename "$CURDIR")
echo "Current directory: $(pwd)"
NEWDIR=$(pwd)

sudo ip link add veth0af_xdp type veth peer name veth1af_xdp
sudo ip link set veth0af_xdp up && sudo ip link set veth1af_xdp up
sudo ip link set veth0af_xdp promisc on
sudo ip link set veth1af_xdp promisc on


sudo ip link add veth0dpdk type veth peer name veth1dpdk
sudo ip link set veth0dpdk up && sudo ip link set veth1dpdk up
sudo ip link set veth0dpdk promisc on
sudo ip link set veth1dpdk promisc on

sudo env LD_LIBRARY_PATH=/usr/local/lib64 "$CARGO" test


sudo ip link del veth0af_xdp
# sudo ip link del veth1af_xdp

sudo ip link del veth0dpdk
# sudo ip link del veth1dpdk

cd "$CURDIR"
sudo rm -rf "$TMPDIR"
