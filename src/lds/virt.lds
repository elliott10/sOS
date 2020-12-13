/*
linker 可理解的架构名称riscv

然后使用参数-mabi=lp64 -march=rv64gc
*/
OUTPUT_ARCH( "riscv" )


/*
在boot.S中的入口"_start", cpu指令开始执行的地方

接下来将会把"_start"放置在0x80000000地址的开头, 这是很多rsic-v板子开始执行的地方
*/
ENTRY( _start )

/*
"ram"包含了一个section可写可执行可分配，"!"倒置 只读和初始化;
ORIGIN是内存地址0x80000000，
内存大小128M,

也可声明其他内存如QSPI,ROM
*/
MEMORY
{
  ram   (wxa!ri) : ORIGIN = 0x80000000, LENGTH = 128M
}
/*
"program headers"
text - cpu指令（可执行节section）
data - 全局已初始化变量
bss  - 全局未初始化变量(全部会被boot.S置成0)

PT_LOAD linker把section节从文件载入进内存；
PT_DYNAMIC
PT_INTERP
PT_NULL 

*/

PHDRS
{
  text PT_LOAD;
  data PT_LOAD;
  bss PT_LOAD;
}

SECTIONS
{
/*
内存布局的第一部分是text节，内存开始地址是0x80000000

PROVIDE定义一个可读取符号
'.'表示当前内存地址,可能变动

也可指定具体的被包含进来的目标文件，
如 boot.o(.text.init)

结尾的":text"告诉linker把这个.text放进PHDRS的text
">ram"告诉linker把".text"放进内存的"ram"

*/
  .text : {
    PROVIDE(_text_start = .);
    *(.text.init)
    *(.text .text.*)
    PROVIDE(_text_end = .);
  } >ram AT>ram :text
   PROVIDE(_global_pointer = .);
  .rodata : {
    PROVIDE(_rodata_start = .);
    *(.rodata .rodata.*)
    PROVIDE(_rodata_end = .);
  } >ram AT>ram :text

  .data : {
    . = ALIGN(4096); /* 当前内存地址对齐到4K*/
    PROVIDE(_data_start = .);
    *(.sdata .sdata.*) *(.data .data.*)
    PROVIDE(_data_end = .);
  } >ram AT>ram :data

  .bss :{
    PROVIDE(_bss_start = .);
    *(.sbss .sbss.*) *(.bss .bss.*)
    PROVIDE(_bss_end = .);
  } >ram AT>ram :bss

  PROVIDE(_memory_start = ORIGIN(ram));
  /*
  stack栈的增长是从高地址到低地址
  */
  PROVIDE(_stack_start = _bss_end);
  PROVIDE(_stack_end = _bss_end + 0x80000);
  PROVIDE(_memory_end = ORIGIN(ram) + LENGTH(ram));
  /*
  heap堆地址在这不进行对齐, 让kernel来决定

  */
  PROVIDE(_heap_start = _stack_end);
  PROVIDE(_heap_size = _memory_end - _heap_start);
}
