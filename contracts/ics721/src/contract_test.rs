#[cfg(test)]
mod contact_testing {
    use super::super::*;
    use crate::test_helpers::*;

    use cosmwasm_std::testing::mock_env;
    use cosmwasm_std::IbcEndpoint;
    use cosmwasm_std::{from_binary, to_binary, Attribute, Coin, StdError};
    use cw2::{get_contract_version, ContractVersion};
    use cw20_ics20::state::ChannelInfo;

    use cosmwasm_std::testing::mock_dependencies;

    fn test_setup(deps: Deps, channel_0: String, channel_1: String) {
        let raw_list = query(deps, mock_env(), QueryMsg::ListChannels {}).unwrap();
        let list_res: ListChannelsResponse = from_binary(&raw_list).unwrap();
        assert_eq!(2, list_res.channels.len());
        assert_eq!(mock_channel_info(&channel_0), list_res.channels[0]);
        assert_eq!(mock_channel_info(&channel_1), list_res.channels[1]);
    }

    #[test]
    fn test_query_success() {
        let deps = setup(&["channel-3", "channel-7"]);
        test_setup(
            deps.as_ref(),
            "channel-3".to_string(),
            "channel-7".to_string(),
        );

        let raw_channel = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::Channel {
                id: "channel-3".to_string(),
            },
        )
        .unwrap();
        let chan_res: ChannelResponse = from_binary(&raw_channel).unwrap();
        assert_eq!(chan_res.info, mock_channel_info("channel-3"));
        assert_eq!(0, chan_res.class_ids.len());
    }
    #[test]
    fn test_query_fail() {
        let deps = setup(&["channel-3", "channel-7"]);
        test_setup(
            deps.as_ref(),
            "channel-3".to_string(),
            "channel-7".to_string(),
        );

        let err = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::Channel {
                id: "channel-10".to_string(),
            },
        )
        .unwrap_err();
        assert_eq!(err, StdError::not_found("cw20_ics20::state::ChannelInfo"));
    }

    #[test]
    fn test_query_channel_list_success() {
        let deps = setup(&["channel-3", "channel-7"]);
        test_setup(
            deps.as_ref(),
            "channel-3".to_string(),
            "channel-7".to_string(),
        );

        let result = query_list(deps.as_ref());

        let expected_list: StdResult<ListChannelsResponse> = Ok(ListChannelsResponse {
            channels: vec![
                ChannelInfo {
                    id: "channel-3".to_string(),
                    counterparty_endpoint: IbcEndpoint {
                        port_id: "transfer-nft".to_string(),
                        channel_id: "channel-35".to_string(),
                    },
                    connection_id: "connection-2".to_string(),
                },
                ChannelInfo {
                    id: "channel-7".to_string(),
                    counterparty_endpoint: IbcEndpoint {
                        port_id: "transfer-nft".to_string(),
                        channel_id: "channel-75".to_string(),
                    },
                    connection_id: "connection-2".to_string(),
                },
            ],
        });
        assert_eq!(result, expected_list);
    }
    #[test]
    fn test_query_channel_list_empty() {
        let mut deps = setup(&["channel-3", "channel-7"]);
        test_setup(
            deps.as_ref(),
            "channel-3".to_string(),
            "channel-7".to_string(),
        );

        CHANNEL_INFO.remove(&mut deps.storage, "channel-3");
        CHANNEL_INFO.remove(&mut deps.storage, "channel-7");
        let result = query_list(deps.as_ref());

        let expected_list: StdResult<ListChannelsResponse> =
            Ok(ListChannelsResponse { channels: vec![] });
        assert_eq!(result, expected_list);
    }

    #[test]
    fn test_query_channel_success() {
        let mut deps = setup(&["channel-3", "channel-7"]);
        test_setup(
            deps.as_ref(),
            "channel-3".to_string(),
            "channel-7".to_string(),
        );

        let info = ChannelInfo {
            id: "channel-1".to_string(),
            counterparty_endpoint: IbcEndpoint {
                port_id: ("counterparty-port1".to_string()),
                channel_id: ("counterparty-channel-1".to_string()),
            },
            connection_id: "connection-id-1".to_string(),
        };
        CHANNEL_INFO
            .save(deps.as_mut().storage, "99", &info)
            .unwrap();

        let contract_addr = "abc/123/collection-addr";

        CHANNEL_STATE
            .save(
                deps.as_mut().storage,
                ("99", contract_addr, "1"),
                &cosmwasm_std::Empty {},
            )
            .unwrap();

        let result = query_channel(deps.as_ref(), "99".to_string());
        let expected_response = Ok(ChannelResponse {
            info: ChannelInfo {
                id: "channel-1".to_string(),
                counterparty_endpoint: IbcEndpoint {
                    port_id: "counterparty-port1".to_string(),
                    channel_id: "counterparty-channel-1".to_string(),
                },
                connection_id: "connection-id-1".to_string(),
            },
            class_ids: vec!["abc/123/collection-addr".to_string()],
        });
        assert_eq!(result, expected_response);
    }

    #[test]
    fn test_query_channel_not_found_error() {
        let mut deps = setup(&["channel-3", "channel-7"]);
        test_setup(
            deps.as_ref(),
            "channel-3".to_string(),
            "channel-7".to_string(),
        );

        let info = ChannelInfo {
            id: "channel-1".to_string(),
            counterparty_endpoint: IbcEndpoint {
                port_id: ("counterparty-port1".to_string()),
                channel_id: ("counterparty-channel-1".to_string()),
            },
            connection_id: "connection-id-1".to_string(),
        };
        CHANNEL_INFO
            .save(deps.as_mut().storage, "99", &info)
            .unwrap();

        let contract_addr = "abc/123/collection-addr";

        CHANNEL_STATE
            .save(
                deps.as_mut().storage,
                ("99", contract_addr, "1"),
                &cosmwasm_std::Empty {},
            )
            .unwrap();

        let result = query_channel(deps.as_ref(), "101".to_string());
        let expected_response = Err(StdError::NotFound {
            kind: "cw20_ics20::state::ChannelInfo".to_string(),
        });
        assert_eq!(result, expected_response);
    }

    #[test]
    fn test_query_channel_duplicates_filtered() {
        let mut deps = setup(&["channel-3", "channel-7"]);
        test_setup(
            deps.as_ref(),
            "channel-3".to_string(),
            "channel-7".to_string(),
        );

        let info = ChannelInfo {
            id: "channel-1".to_string(),
            counterparty_endpoint: IbcEndpoint {
                port_id: ("counterparty-port1".to_string()),
                channel_id: ("counterparty-channel-1".to_string()),
            },
            connection_id: "connection-id-1".to_string(),
        };
        CHANNEL_INFO
            .save(deps.as_mut().storage, "99", &info)
            .unwrap();

        let contract_addr = "abc/123/collection-addr";

        CHANNEL_STATE
            .save(
                deps.as_mut().storage,
                ("99", contract_addr, "1"),
                &cosmwasm_std::Empty {},
            )
            .unwrap();

        CHANNEL_STATE
            .save(
                deps.as_mut().storage,
                ("99", contract_addr, "2"),
                &cosmwasm_std::Empty {},
            )
            .unwrap();

        let result = query_channel(deps.as_ref(), "99".to_string());
        let expected_response = Ok(ChannelResponse {
            info: ChannelInfo {
                id: "channel-1".to_string(),
                counterparty_endpoint: IbcEndpoint {
                    port_id: "counterparty-port1".to_string(),
                    channel_id: "counterparty-channel-1".to_string(),
                },
                connection_id: "connection-id-1".to_string(),
            },
            class_ids: vec!["abc/123/collection-addr".to_string()],
        });
        assert_eq!(result, expected_response);
    }

    #[test]
    fn test_query_channel_multiple_success() {
        let mut deps = setup(&["channel-3", "channel-7"]);
        test_setup(
            deps.as_ref(),
            "channel-3".to_string(),
            "channel-7".to_string(),
        );

        let info = ChannelInfo {
            id: "channel-1".to_string(),
            counterparty_endpoint: IbcEndpoint {
                port_id: ("counterparty-port1".to_string()),
                channel_id: ("counterparty-channel-1".to_string()),
            },
            connection_id: "connection-id-1".to_string(),
        };
        CHANNEL_INFO
            .save(deps.as_mut().storage, "99", &info)
            .unwrap();

        let contract_addr = "abc/123/collection-addr";
        let contract_addr2 = "abc/456/collection-addr";

        CHANNEL_STATE
            .save(
                deps.as_mut().storage,
                ("99", contract_addr, "1"),
                &cosmwasm_std::Empty {},
            )
            .unwrap();

        CHANNEL_STATE
            .save(
                deps.as_mut().storage,
                ("99", contract_addr2, "1"),
                &cosmwasm_std::Empty {},
            )
            .unwrap();

        let result = query_channel(deps.as_ref(), "99".to_string());
        let expected_response = Ok(ChannelResponse {
            info: ChannelInfo {
                id: "channel-1".to_string(),
                counterparty_endpoint: IbcEndpoint {
                    port_id: "counterparty-port1".to_string(),
                    channel_id: "counterparty-channel-1".to_string(),
                },
                connection_id: "connection-id-1".to_string(),
            },
            class_ids: vec![
                "abc/123/collection-addr".to_string(),
                "abc/456/collection-addr".to_string(),
            ],
        });
        assert_eq!(result, expected_response);
    }

    #[test]
    fn test_instantiate() {
        let mut deps = mock_dependencies();

        let sender_address: Addr = Addr::unchecked("stars1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n");
        let coin = Coin::new(128, "testing-coin");
        let initial_funds = vec![coin];
        let info_msg: MessageInfo = MessageInfo {
            sender: sender_address,
            funds: initial_funds,
        };
        let instantiate_msg = InstantiateMsg {
            default_timeout: 1000,
        };

        let contract_version_before = get_contract_version(&deps.storage).unwrap_err();
        let expected_contract_version_before: StdError = StdError::NotFound {
            kind: "cw2::ContractVersion".to_string(),
        };
        assert_eq!(contract_version_before, expected_contract_version_before);

        let result = instantiate(deps.as_mut(), mock_env(), info_msg, instantiate_msg);
        let expected_result: Result<Response, ContractError> = Ok(Response::default());
        assert_eq!(result.unwrap(), expected_result.unwrap());

        let contract_version_after = get_contract_version(&deps.storage);
        let expected_contract_version = Ok(ContractVersion {
            contract: "crates.io:sg721-ics721".to_string(),
            version: "0.1.0".to_string(),
        });
        assert_eq!(contract_version_after, expected_contract_version);
        let expected_config = Some(Config {
            default_timeout: 1000,
        });
        assert_eq!(CONFIG.may_load(&deps.storage), Ok(expected_config));
    }

    #[test]
    fn test_execute_transfer_success() {
        let mut deps = setup(&["channel-3", "channel-7"]);
        test_setup(
            deps.as_ref(),
            "channel-3".to_string(),
            "channel-7".to_string(),
        );

        let transfer_msg = TransferMsg {
            channel: "channel-3".to_string(),
            class_id: "abc/123/collection-addr".to_string(),
            class_uri: Some("abc/456/collection-addr".to_string()),
            token_ids: vec!["1".to_string(), "2".to_string(), "3".to_string()],
            token_uris: vec![
                "https://metadata-url.com/my-metadata1".to_string(),
                "https://metadata-url.com/my-metadata2".to_string(),
                "https://metadata-url.com/my-metadata3".to_string(),
            ],
            remote_address: "stars1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n".to_string(),
            timeout: Some(1000),
        };
        let sender_address: Addr = Addr::unchecked("wasm1fucynrfkrt684pm8jrt8la5h2csvs5cnldcgqc");
        let result = execute_transfer(deps.as_mut(), mock_env(), transfer_msg, sender_address);
        let expected_result = [
            Attribute {
                key: "action".into(),
                value: "transfer".into(),
            },
            Attribute {
                key: "sender".into(),
                value: "wasm1fucynrfkrt684pm8jrt8la5h2csvs5cnldcgqc".into(),
            },
            Attribute {
                key: "receiver".into(),
                value: "stars1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n".into(),
            },
            Attribute {
                key: "class_id".into(),
                value: "cosmos2contract".into(),
            },
            Attribute {
                key: "token_ids".into(),
                value: "1,2,3".into(),
            },
        ];
        assert_eq!(result.unwrap().attributes, expected_result);
    }

    #[test]
    fn test_execute_receive_success() {
        let mut deps = setup(&["channel-3", "channel-7"]);
        test_setup(
            deps.as_ref(),
            "channel-3".to_string(),
            "channel-7".to_string(),
        );

        let sender_address_str = "wasm1fucynrfkrt684pm8jrt8la5h2csvs5cnldcgqc";
        let sender_address: Addr = Addr::unchecked(sender_address_str);

        let transfer_msg = TransferMsg {
            channel: "channel-3".to_string(),
            class_id: "abc/123/collection-addr".to_string(),
            class_uri: Some("abc/456/collection-addr".to_string()),
            token_ids: vec!["1".to_string(), "2".to_string(), "3".to_string()],
            token_uris: vec![
                "https://metadata-url.com/my-metadata1".to_string(),
                "https://metadata-url.com/my-metadata2".to_string(),
                "https://metadata-url.com/my-metadata3".to_string(),
            ],
            remote_address: "stars1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n".to_string(),
            timeout: Some(1000),
        };

        let cw721_receive_msg = Cw721ReceiveMsg {
            sender: sender_address_str.to_string(),
            token_id: "1".to_string(),
            msg: to_binary(&transfer_msg).unwrap(),
        };

        let initial_funds = vec![];
        let info_msg: MessageInfo = MessageInfo {
            sender: sender_address,
            funds: initial_funds,
        };

        let result = execute_receive(deps.as_mut(), mock_env(), info_msg, cw721_receive_msg);
        let expected_result = [
            Attribute {
                key: "action".into(),
                value: "transfer".into(),
            },
            Attribute {
                key: "sender".into(),
                value: "wasm1fucynrfkrt684pm8jrt8la5h2csvs5cnldcgqc".into(),
            },
            Attribute {
                key: "receiver".into(),
                value: "stars1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n".into(),
            },
            Attribute {
                key: "class_id".into(),
                value: "cosmos2contract".into(),
            },
            Attribute {
                key: "token_ids".into(),
                value: "1,2,3".into(),
            },
        ];
        let result_clone = result.unwrap();
        assert_eq!(result_clone.attributes, expected_result);
        assert_eq!(result_clone.messages.len(), 1);
    }

    #[test]
    fn test_execute_receive_nonpayable_fail() {
        let mut deps = setup(&["channel-3", "channel-7"]);
        test_setup(
            deps.as_ref(),
            "channel-3".to_string(),
            "channel-7".to_string(),
        );

        let sender_address_str = "wasm1fucynrfkrt684pm8jrt8la5h2csvs5cnldcgqc";
        let sender_address: Addr = Addr::unchecked(sender_address_str);

        let transfer_msg = TransferMsg {
            channel: "channel-3".to_string(),
            class_id: "abc/123/collection-addr".to_string(),
            class_uri: Some("abc/456/collection-addr".to_string()),
            token_ids: vec!["1".to_string(), "2".to_string(), "3".to_string()],
            token_uris: vec![
                "https://metadata-url.com/my-metadata1".to_string(),
                "https://metadata-url.com/my-metadata2".to_string(),
                "https://metadata-url.com/my-metadata3".to_string(),
            ],
            remote_address: "stars1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n".to_string(),
            timeout: Some(1000),
        };

        let cw721_receive_msg = Cw721ReceiveMsg {
            sender: sender_address_str.to_string(),
            token_id: "1".to_string(),
            msg: to_binary(&transfer_msg).unwrap(),
        };

        let coin = Coin::new(128, "testing-coin");
        let initial_funds = vec![coin];
        let info_msg: MessageInfo = MessageInfo {
            sender: sender_address,
            funds: initial_funds,
        };
        use cw20_ics20::ContractError::Payment;
        use cw_utils::PaymentError;
        let result =
            execute_receive(deps.as_mut(), mock_env(), info_msg, cw721_receive_msg).unwrap_err();

        let expected_result: cw20_ics20::ContractError = Payment(PaymentError::NonPayable {});
        assert_eq!(result.to_string(), expected_result.to_string());
    }

    #[test]
    fn test_execute_to_execute_receive_success() {
        let mut deps = setup(&["channel-3", "channel-7"]);
        test_setup(
            deps.as_ref(),
            "channel-3".to_string(),
            "channel-7".to_string(),
        );

        let sender_address_str = "wasm1fucynrfkrt684pm8jrt8la5h2csvs5cnldcgqc";
        let sender_address: Addr = Addr::unchecked(sender_address_str);

        let transfer_msg = TransferMsg {
            channel: "channel-3".to_string(),
            class_id: "abc/123/collection-addr".to_string(),
            class_uri: Some("abc/456/collection-addr".to_string()),
            token_ids: vec!["1".to_string(), "2".to_string(), "3".to_string()],
            token_uris: vec![
                "https://metadata-url.com/my-metadata1".to_string(),
                "https://metadata-url.com/my-metadata2".to_string(),
                "https://metadata-url.com/my-metadata3".to_string(),
            ],
            remote_address: "stars1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n".to_string(),
            timeout: Some(1000),
        };

        let cw721_receive_msg = ExecuteMsg::Receive(Cw721ReceiveMsg {
            sender: "sender_address_receive_path".to_string(),
            token_id: "1".to_string(),
            msg: to_binary(&transfer_msg).unwrap(),
        });

        let initial_funds = vec![];
        let info_msg: MessageInfo = MessageInfo {
            sender: sender_address,
            funds: initial_funds,
        };

        let result = execute(deps.as_mut(), mock_env(), info_msg, cw721_receive_msg);
        let expected_result = [
            Attribute {
                key: "action".into(),
                value: "transfer".into(),
            },
            Attribute {
                key: "sender".into(),
                value: "sender_address_receive_path".into(),
            },
            Attribute {
                key: "receiver".into(),
                value: "stars1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n".into(),
            },
            Attribute {
                key: "class_id".into(),
                value: "cosmos2contract".into(),
            },
            Attribute {
                key: "token_ids".into(),
                value: "1,2,3".into(),
            },
        ];
        let result_clone = result.unwrap();
        assert_eq!(result_clone.attributes, expected_result);
        assert_eq!(result_clone.messages.len(), 1);
    }

    #[test]
    fn test_execute_to_execute_transfer_success() {
        let mut deps = setup(&["channel-3", "channel-7"]);
        test_setup(
            deps.as_ref(),
            "channel-3".to_string(),
            "channel-7".to_string(),
        );

        let transfer_msg = ExecuteMsg::Transfer(TransferMsg {
            channel: "channel-3".to_string(),
            class_id: "abc/123/collection-addr".to_string(),
            class_uri: Some("abc/456/collection-addr".to_string()),
            token_ids: vec!["1".to_string(), "2".to_string(), "3".to_string()],
            token_uris: vec![
                "https://metadata-url.com/my-metadata1".to_string(),
                "https://metadata-url.com/my-metadata2".to_string(),
                "https://metadata-url.com/my-metadata3".to_string(),
            ],
            remote_address: "stars1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n".to_string(),
            timeout: Some(1000),
        });
        let sender_address: Addr = Addr::unchecked("wasm1fucynrfkrt684pm8jrt8la5h2csvs5cnldcgqc");
        let initial_funds = vec![];
        let info_msg: MessageInfo = MessageInfo {
            sender: sender_address,
            funds: initial_funds,
        };

        let result = execute(deps.as_mut(), mock_env(), info_msg, transfer_msg);
        let expected_results = [
            Attribute {
                key: "action".into(),
                value: "transfer".into(),
            },
            Attribute {
                key: "sender".into(),
                value: "wasm1fucynrfkrt684pm8jrt8la5h2csvs5cnldcgqc".into(),
            },
            Attribute {
                key: "receiver".into(),
                value: "stars1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n".into(),
            },
            Attribute {
                key: "class_id".into(),
                value: "cosmos2contract".into(),
            },
            Attribute {
                key: "token_ids".into(),
                value: "1,2,3".into(),
            },
        ];
        assert_eq!(result.unwrap().attributes, expected_results);
    }

    #[test]
    fn test_transfer_packet_fail() {
        // TODO need to implement packet validation in order to fail the transfer
    }
}
