module.exports = async function confirmOrder(ctx) {
  const drinkId = ctx.arguments.drinkId
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
