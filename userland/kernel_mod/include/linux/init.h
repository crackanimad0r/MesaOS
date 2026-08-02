#ifndef _LINUX_INIT_H
#define _LINUX_INIT_H

#define __init __attribute__((__section__(".init.text")))
#define __exit __attribute__((__section__(".exit.text")))

#define module_init(x)    extern int init_module(void) __attribute__((alias(#x)));
#define module_exit(x)    extern void cleanup_module(void) __attribute__((alias(#x)));

#endif
