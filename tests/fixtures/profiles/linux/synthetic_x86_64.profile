aegishv-linux-profile-v1
os=linux
arch=x86_64
kernel_release=6.8.0-aegishv-synthetic
kernel_build=synthetic-test-build
variant=test
kaslr=slide-known
kaslr_slide=0x100000

symbol=start_kernel,0xffffffff81000000,0x120
symbol=sys_call_table,0xffffffff81200000

offset=task_struct,pid,0x430,0x4
offset=task_struct,comm,0x738,0x10

syscall=0,read,__x64_sys_read
syscall=1,write,__x64_sys_write
