# ( Netmap
sudo modprobe /home/leonardogiovannoni/Src/Net/netmap/netmap.ko
sudo chgrp netmap /dev/netmap
sudo chmod 660 /dev/netmap
# )
# ( DPDK
echo 1024 | sudo tee /proc/sys/vm/nr_hugepages
sudo mkdir -p /mnt/huge
sudo mount -t hugetlbfs nodev /mnt/huge
# )
