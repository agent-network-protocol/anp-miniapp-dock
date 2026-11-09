Component({
  data: {
    imageHandle: '',
    fileHandle: '',
    previewImage: '',
    poster: '',
    boundary: 'host-boundary'
  },
  lifetimes: {
    created() {
      const modelCtx = wx.modelContext.getContext(this)
      modelCtx.on(wx.modelContext.NotificationType.Result, (data) => {
        const result = data.result || {}
        const structured = result.structuredContent || {}
        const media = structured.media || {}
        const boundary = structured.boundary || {}
        this.setData({
          imageHandle: media.imageHandle || '',
          fileHandle: media.fileHandle || '',
          previewImage: media.previewImage || '',
          poster: media.poster || '',
          boundary: `${boundary.provider || 'wx.chooseMedia'}:${boundary.status || 'host-boundary'}`
        })
      })
    }
  },
  methods: {
    approve() {
      wx.modelContext.getContext(this).sendFollowUpMessage({
        content: [
          { type: 'text', text: 'Approve mock media handles' },
          {
            type: 'api/call',
            data: {
              name: 'reviewMedia',
              arguments: {
                imageHandle: this.data.imageHandle,
                fileHandle: this.data.fileHandle
              }
            }
          }
        ]
      })
    },
    onImageLoad() {},
    onImageError() {}
  }
})

