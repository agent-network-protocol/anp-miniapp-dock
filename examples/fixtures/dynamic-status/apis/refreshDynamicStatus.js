module.exports = async function refreshDynamicStatus() {
  return {
    isError: false,
    content: [{ type: 'text', text: 'Mock dynamic status is ready' }],
    structuredContent: {
      orderId: 'order_demo_001',
      status: 'pending'
    },
    _meta: {
      risk: 'l2',
      fixture: 'dynamic-status',
      mockOnly: true
    }
  }
}

