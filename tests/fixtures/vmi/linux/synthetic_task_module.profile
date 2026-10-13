aegishv-linux-profile-v1
os=linux
arch=x86_64
kernel_release=6.8.0-aegishv-synthetic
kernel_build=synthetic-task-module-build
variant=task-module
kaslr=fixed

symbol=_stext,0xffffffff81000000
symbol=_etext,0xffffffff81008000
symbol=init_task,0xffff888000010000
symbol=modules,0xffff888000020000
symbol=sys_call_table,0xffffffff81004000
symbol=entry_SYSCALL_64,0xffffffff81005000

offset=task_struct,tasks,0x0,0x10
offset=task_struct,pid,0x10,0x4
offset=task_struct,comm,0x18,0x10
offset=task_struct,mm,0x30,0x8

offset=module,list,0x0,0x10
offset=module,name,0x10,0x38
offset=module,core_layout_base,0x50,0x8
offset=module,core_layout_size,0x58,0x8

syscall=0,read,0xffffffff81006000
syscall=1,write,0xffffffff81006100
syscall=59,execve,0xffffffff81006200
