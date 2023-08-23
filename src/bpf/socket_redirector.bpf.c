#include "vmlinux.h"
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>
#include "socket_redirector.h"

SEC("sk_lookup/redirector")
int redirector(struct bpf_sk_lookup *ctx)
{
    __u32 ipv4;
    __u32 *found_ip;
    const __u32 zero = 0;
    struct bpf_sock *sk;
    int err;

    ipv4 = bpf_ntohl(ctx->local_ip4);
    found_ip = bpf_map_lookup_elem(&ips, &ipv4);

    if (!found_ip) return SK_PASS;

    sk = bpf_map_lookup_elem(&sockets, &zero);

    if (!sk) return SK_DROP;

    err = bpf_sk_assign(ctx, sk, 0);
    bpf_sk_release(sk);

    return err ? SK_DROP : SK_PASS;
}

SEC("license") const char __license[] = "GPL";
