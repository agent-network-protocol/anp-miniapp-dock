module.exports = async function prepareAddressForm() {
  return {
    isError: false,
    content: [{ type: 'text', text: 'Mock address form is ready' }],
    structuredContent: {
      form: {
        recipient: 'Demo Recipient',
        note: 'Use opaque address handle only',
        slots: ['09:00-10:00', '10:00-11:00'],
        selectedSlot: 1,
        addressHandle: 'addr_handle_demo_001'
      },
      boundary: {
        provider: 'wx.chooseAddress',
        status: 'host-boundary',
        consent: 'required'
      }
    },
    _meta: {
      risk: 'l4',
      fixture: 'address-form',
      mockOnly: true
    }
  }
}

