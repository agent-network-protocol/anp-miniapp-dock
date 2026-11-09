Component({
  data: {
    orderId: '',
    status: 'pending',
    pollCount: 0,
    timerState: 'not-run',
    safeHeader: '',
    errorCode: ''
  },
  lifetimes: {
    created() {
      const modelCtx = wx.modelContext.getContext(this)
      modelCtx.on(wx.modelContext.NotificationType.Result, (data) => {
        const structured = (data.result && data.result.structuredContent) || {}
        this.setData({
          orderId: structured.orderId || 'order_demo_001',
          status: structured.status || 'pending'
        })
      })
    },
    async attached() {
      try {
        const response = await wx.request({
          url: 'https://merchant.example.invalid/status',
          method: 'POST',
          data: { orderId: this.data.orderId },
          header: { 'x-fixture': 'dynamic-status' }
        })
        this.setData({
          status: response.data.status,
          pollCount: this.data.pollCount + 1,
          safeHeader: response.header['x-fixture-safe'] || ''
        })
        setTimeout(() => this.setData({ timerState: 'timer-flushed' }), 0)
      } catch (error) {
        this.setData({
          status: 'request-denied',
          errorCode: error.code || 'request_failed'
        })
      }
    }
  },
  methods: {
    refresh() {
      wx.modelContext.getContext(this).sendFollowUpMessage({
        content: [
          { type: 'text', text: 'Refresh mock status' },
          {
            type: 'api/call',
            data: {
              name: 'refreshDynamicStatus',
              arguments: { orderId: this.data.orderId }
            }
          }
        ]
      })
    }
  }
})

