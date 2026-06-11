module.exports = async function payOrder(ctx) {
  const orderId = ctx.arguments.orderId
  const remote = globalThis.__coffeeRemote

  if (remote && remote.shouldUseRemote(ctx)) {
    const payment = await remote.request(ctx, '/api/order/pay', {
      method: 'POST',
      data: { orderId }
    })

    return {
      isError: false,
      content: [{ type: 'text', text: `Payment completed for ${orderId} from localhost coffee service` }],
      structuredContent: {
        orderId: payment.orderId || orderId,
        status: payment.status
      },
      _meta: Object.assign({
        risk: 'payment',
      }, remote.authMeta(ctx))
    }
  }

  return {
    isError: false,
    content: [{ type: 'text', text: `Payment completed for ${orderId}` }],
    structuredContent: {
      orderId,
      status: 'paid'
    },
    _meta: {
      risk: 'payment'
    }
  }
}
