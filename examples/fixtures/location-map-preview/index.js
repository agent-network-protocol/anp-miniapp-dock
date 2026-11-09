const skill = wx.modelContext.createSkill(__dirname)

skill.registerAPI('prepareLocationMap', require('./apis/prepareLocationMap'))

module.exports = skill

