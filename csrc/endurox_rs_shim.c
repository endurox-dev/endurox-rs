#include <errno.h>
#include <ndrx_config.h>

#if defined(EX_USE_EPOLL) && EX_USE_EPOLL == 1
#include <atmi_int.h>
#include <atmi_tls.h>
#endif

int endurox_rs_reply_queue_fd(void)
{
#if defined(EX_USE_EPOLL) && EX_USE_EPOLL == 1
    ATMI_TLS_ENTRY;

    if (NULL == G_atmi_tls || !G_atmi_tls->G_atmi_is_init ||
            0 == G_atmi_tls->G_atmi_conf.reply_q)
    {
        errno = ENODEV;
        return -1;
    }

    return (int)G_atmi_tls->G_atmi_conf.reply_q;
#else
    errno = ENOTSUP;
    return -1;
#endif
}
