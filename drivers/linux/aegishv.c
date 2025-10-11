// SPDX-License-Identifier: MIT
#include <linux/module.h>
#include <linux/fs.h>
#include <linux/cdev.h>
#include <linux/uaccess.h>
#include <linux/slab.h>
#include <linux/mm.h>

#define DEV_NAME "aegishv"
#define RING_REC_SZ 128
#define RING_CAP 4096

static dev_t devno;
static struct cdev cdev_;
static struct class* cls;

static char* ring;
static atomic_t w_idx = ATOMIC_INIT(0);
static atomic_t r_idx = ATOMIC_INIT(0);

static ssize_t hv_read(struct file* f, char __user* buf, size_t len, loff_t* off) {
    int r = atomic_read(&r_idx);
    int w = atomic_read(&w_idx);
    if (r == w) return 0;
    if (len < RING_REC_SZ) return -EINVAL;
    if (copy_to_user(buf, ring + (r % RING_CAP) * RING_REC_SZ, RING_REC_SZ)) return -EFAULT;
    atomic_set(&r_idx, (r + 1));
    return RING_REC_SZ;
}

static ssize_t hv_write(struct file* f, const char __user* buf, size_t len, loff_t* off) {
    if (len < RING_REC_SZ) return -EINVAL;
    int w = atomic_read(&w_idx);
    if (copy_from_user(ring + (w % RING_CAP) * RING_REC_SZ, buf, RING_REC_SZ)) return -EFAULT;
    atomic_set(&w_idx, w + 1);
    return RING_REC_SZ;
}

static const struct file_operations fops = {
    .owner = THIS_MODULE,
    .read = hv_read,
    .write = hv_write,
};

static int __init hv_init(void) {
    ring = kzalloc(RING_CAP * RING_REC_SZ, GFP_KERNEL);
    if (!ring) return -ENOMEM;
    if (alloc_chrdev_region(&devno, 0, 1, DEV_NAME)) return -EBUSY;
    cdev_init(&cdev_, &fops);
    if (cdev_add(&cdev_, devno, 1)) return -EBUSY;
    cls = class_create(THIS_MODULE, DEV_NAME);
    device_create(cls, NULL, devno, NULL, DEV_NAME);
    pr_info("aegishv: loaded\n");
    return 0;
}

static void __exit hv_exit(void) {
    device_destroy(cls, devno);
    class_destroy(cls);
    cdev_del(&cdev_);
    unregister_chrdev_region(devno, 1);
    kfree(ring);
    pr_info("aegishv: unloaded\n");
}

module_init(hv_init);
module_exit(hv_exit);
MODULE_LICENSE("MIT");
