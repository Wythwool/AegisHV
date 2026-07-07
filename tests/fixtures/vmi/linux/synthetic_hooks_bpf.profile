aegishv-linux-profile-v1
os=linux
arch=x86_64
kernel_release=6.8.0-aegishv-synthetic
kernel_build=synthetic-test-build
variant=hooks-bpf
kaslr=fixed

symbol=_stext,0xffffffff81000000
symbol=_etext,0xffffffff81005000
symbol=ftrace_ops_list,0xffff888000001000
symbol=kprobe_table,0xffff888000002000
symbol=bpf_prog_list,0xffff888000003000

offset=ftrace_ops,list,0x0,0x10
offset=ftrace_ops,func,0x10,0x8
offset=ftrace_ops,flags,0x18,0x4

offset=kprobe,hlist,0x0,0x10
offset=kprobe,addr,0x10,0x8
offset=kprobe,pre_handler,0x18,0x8
offset=kprobe,post_handler,0x20,0x8
offset=kprobe,fault_handler,0x28,0x8

offset=bpf_prog,list,0x0,0x10
offset=bpf_prog,aux,0x10,0x8
offset=bpf_prog,type,0x18,0x4
offset=bpf_prog,bpf_func,0x20,0x8
offset=bpf_prog,jited_len,0x28,0x4
offset=bpf_prog_aux,id,0x0,0x4
offset=bpf_prog_aux,name,0x8,0x10
