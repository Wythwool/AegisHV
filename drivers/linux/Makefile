obj-m += aegishv.o

all:
\t$(MAKE) -C /lib/modules/$(shell uname -r)/build M=$(PWD) modules

clean:
\t$(MAKE) -C /lib/modules/$(shell uname -r)/build M=$(PWD) clean
