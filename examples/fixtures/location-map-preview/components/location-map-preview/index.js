Component({
  data: {
    region: '',
    locationToken: '',
    providerStatus: 'fail-closed',
    fallbackReason: 'host_location_provider_required'
  },
  lifetimes: {
    created() {
      const modelCtx = wx.modelContext.getContext(this)
      modelCtx.on(wx.modelContext.NotificationType.Result, (data) => {
        const structured = (data.result && data.result.structuredContent) || {}
        const location = structured.location || {}
        this.setData({
          region: location.region || 'mock-region-downtown',
          locationToken: location.locationToken || '',
          providerStatus: location.providerStatus || 'fail-closed',
          fallbackReason: location.fallbackReason || 'host_location_provider_required'
        })
      })
    }
  },
  methods: {
    requestLocation() {
      wx.modelContext.getContext(this).sendFollowUpMessage({
        content: [
          { type: 'text', text: 'Request Host location provider' },
          {
            type: 'api/call',
            data: {
              name: 'prepareLocationMap',
              arguments: { locationToken: this.data.locationToken }
            }
          }
        ]
      })
    }
  }
})

