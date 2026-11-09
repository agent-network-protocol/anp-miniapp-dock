module.exports = async function reviewMedia() {
  return {
    isError: false,
    content: [{ type: 'text', text: 'Mock media handles are ready for review' }],
    structuredContent: {
      media: {
        imageHandle: 'image_handle_demo_001',
        fileHandle: 'file_handle_demo_001',
        previewImage: 'https://static.example.invalid/fixtures/media-preview.png',
        poster: 'https://static.example.invalid/fixtures/media-poster.png'
      },
      boundary: {
        provider: 'wx.chooseMedia',
        status: 'host-boundary',
        handleType: 'opaque'
      }
    },
    _meta: {
      risk: 'l4',
      fixture: 'media-review',
      mockOnly: true
    }
  }
}

