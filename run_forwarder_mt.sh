sudo ./target/debug/examples/forward_mt veth0 v0 pcap 

# NS1 will shoot packets to its interface (veth1), NS2 will receive those packets (in v1) thanks to the default namespace (veth0 -> v0)
# veth1 -- veth0 -- forwarder -- v0 -- v1
sudo ip netns add NS1
sudo ip netns add NS2


# veth0 receiving
sudo ip link add veth0 type veth peer name veth1
sudo ip link set veth1 netns NS1

sudo ip link add v0 type veth peer name v1
sudo ip link set v1 netns NS2

# v0 receiving what veth0 receives, so v1


sudo ip netns exec NS1 /bin/bash
# NS1
./target/debug/examples/pkt-gen -i veth1 --dst-mac ff:ff:ff:ff:ff:ff --src-ip 10.0.0.1 --dst-ip 10.1.0.1 pcap


sudo ip netns exec NS2 /bin/bash 
# NS2
env LD_LIBRARY_PATH=/usr/local/lib/x86_64-linux-gnu:/usr/local/lib64 ./target/debug/examples/meter -s 1 --interface v1 pcap 
