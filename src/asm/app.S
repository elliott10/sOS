    .align 4
    .section .rodata
    .global _num_app
_num_app:
    .quad 3
    .quad app_0_start
    .quad app_1_start
    .quad app_2_start
    .quad app_2_end

    .section .rodata
    .global app_0_start
    .global app_0_end
app_0_start:
    .incbin "/home/xly/rust/tmp/hello_world.bin"
app_0_end:
# opensbi也是用.incbin包含FW_PAYLOAD
        

    .section .rodata
    .global app_1_start
    .global app_1_end
app_1_start:
    .incbin "/home/xly/rust/tmp/fantastic_text.bin"
app_1_end:
        

    .section .rodata
    .global app_2_start
    .global app_2_end
app_2_start:
    .incbin "/home/xly/rust/tmp/user_shell.bin"
app_2_end:
        
