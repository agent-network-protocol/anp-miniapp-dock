module.exports = async function prepareLocationMap() {
  return {
    isError: false,
    content: [{ type: 'text', text: 'Mock location preview is ready' }],
    structuredContent: {
      location: {
        region: 'mock-region-downtown',
        locationToken: 'location_handle_demo_001',
        providerStatus: 'fail-closed',
        fallbackReason: 'host_location_provider_required'
      }
    },
    _meta: {
      risk: 'l4',
      fixture: 'location-map-preview',
      mockOnly: true
    }
  }
}

