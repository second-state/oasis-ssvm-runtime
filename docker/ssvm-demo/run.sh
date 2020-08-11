docker build -t secondstate/oasis-ssvm:demo .
docker run -it --rm --name ssvm-demo --security-opt apparmor:unconfined --security-opt seccomp=unconfined secondstate/oasis-ssvm:demo
