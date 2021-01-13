### RISC-V OS using Rust

* From scratch

* Make a hard disk img
```
dd if=/dev/zero of=hdd.dsk count=32 bs=1M
```

* Build
```
make
make run 
```

* Boot

![Boot Screen](pictures/boot.png)

