module.exports = async function confirmOrder(ctx) {
  const drinkId = ctx.arguments.drinkId
  const remote = globalThis.__coffeeRemote

  if (remote && remote.shouldUseRemote(ctx)) {
    const order = await remote.request(ctx, '/api/order/confirm', {
      method: 'POST',
      data: {
        drinkId,
        size: ctx.arguments.size || 'medium',
        sugar: ctx.arguments.sugar || 'less'
      }
    })

    return {
      isError: false,
      content: [{ type: 'text', text: `Confirm order for ${drinkId} from localhost coffee service` }],
      structuredContent: {
        orderId: order.orderId,
        drinkId: order.drinkId || drinkId,
        payable: order.payable
      },
      _meta: Object.assign({
        risk: 'order',
      }, remote.authMeta(ctx))
    }
  }

  return {
    isError: false,
    content: [{ type: 'text', text: `Confirm order for ${drinkId}` }],
    structuredContent: {
      orderId: 'order_demo_001',
      drinkId,
      payable: 18
    },
    _meta: {
      risk: 'order'
    }
  }
}
