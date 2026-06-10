Component({
  data: {
    title: 'Payment result'
  },
  lifetimes: {
    created() {
      const modelCtx = wx.modelContext.getContext(this)
      modelCtx.on(wx.modelContext.NotificationType.Result, (data) => {
        const result = data.result.structuredContent
        this.setData({
          orderId: result.orderId,
          status: result.status
        })
        wx.modelContext.getViewContext(this).expirePreviousCards({
          componentPaths: ['components/order-confirm/index'],
          match: 'latest'
        })
      })
    }
  }
})
