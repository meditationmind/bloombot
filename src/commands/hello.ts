import { SlashCommandBuilder } from "discord.js";

export = {
	data: new SlashCommandBuilder()
		.setName('hello')
		.setDescription('Says hello!'),
	async execute(interaction) {
		await interaction.reply({ content: 'Hey there!' });
	},
};