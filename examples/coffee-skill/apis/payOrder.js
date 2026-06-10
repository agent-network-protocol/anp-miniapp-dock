module.exports = async function payOrder(ctx) {
  const orderId = ctx.arguments.orderId
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
