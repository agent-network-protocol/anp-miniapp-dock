Component({
  data: {
    title: 'Confirm order'
  },
  lifetimes: {
    created() {
      const modelCtx = wx.modelContext.getContext(this)
      modelCtx.on(wx.modelContext.NotificationType.Result, (data) => {
        const result = data.result.structuredContent
        this.setData({
          orderId: result.orderId,
          drinkName: result.drinkId,
          payable: result.payable
        })
      })
    }
  },
  methods: {
    payOrder(e) {
      wx.modelContext.getContext(this).sendFollowUpMessage({
        content: [
          { type: 'text', text: '确认支付' },
          {
            type: 'api/call',
            data: {
              name: 'payOrder',
              arguments: { orderId: e.currentTarget.dataset.orderId }
            }
          }
        ]
      })
    }
  }
})
