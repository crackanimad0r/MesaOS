#include <linux/module.h>
#include <linux/kernel.h>
#include <linux/init.h>

int __init hello(void)
{
    printk(KERN_INFO "Hola mundo!\n");
    return 0;
}

void __exit goodbye(void)
{
    printk(KERN_INFO "Adios, Linux 100% real para nada es una capa de compatibilidad!\n");
}

module_init(hello);
module_exit(goodbye);
