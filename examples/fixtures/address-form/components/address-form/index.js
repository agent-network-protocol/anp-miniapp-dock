Component({
  data: {
    recipient: '',
    note: '',
    slots: [],
    selectedSlot: 0,
    addressHandle: '',
    boundary: 'host-boundary'
  },
  lifetimes: {
    created() {
      const modelCtx = wx.modelContext.getContext(this)
      modelCtx.on(wx.modelContext.NotificationType.Result, (data) => {
        const result = data.result || {}
        const structured = result.structuredContent || {}
        const form = structured.form || {}
        const boundary = structured.boundary || {}
        this.setData({
          recipient: form.recipient || '',
          note: form.note || '',
          slots: form.slots || [],
          selectedSlot: form.selectedSlot || 0,
          addressHandle: form.addressHandle || '',
          boundary: `${boundary.provider || 'wx.chooseAddress'}:${boundary.status || 'host-boundary'}`
        })
      })
    }
  },
  methods: {
    onRecipient() {},
    onNote() {},
    onSlotChange() {},
    submit() {
      wx.modelContext.getContext(this).sendFollowUpMessage({
        content: [
          { type: 'text', text: 'Submit mock address handle' },
          {
            type: 'api/call',
            data: {
              name: 'prepareAddressForm',
              arguments: {
                addressHandle: this.data.addressHandle,
                deliverySlot: this.data.slots[this.data.selectedSlot] || ''
              }
            }
          }
        ]
      })
    }
  }
})

