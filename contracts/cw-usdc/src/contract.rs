use std::fs::Permissions;

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint128,
};
use cw2::set_contract_version;

use osmo_bindings::{OsmosisMsg, OsmosisQuery};
use osmo_bindings_test::OsmosisModule;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, IsFrozenResponse, QueryMsg, SudoMsg};
use crate::state::{
    Config, BLACKLISTED_ADDRESSES, BLACKLISTER_ALLOWANCES, CONFIG, MINTER_ALLOWANCES,
};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cw-usdc";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // TODO trigger CreateDenom msg
    OsmosisMsg::CreateDenom {
        subdenom: msg.subdenom,
    };

    let config = Config {
        owner: info.sender.clone(),
        is_frozen: false,
        denom: String::from("TODO"), // TODO: use denom from actual message
    };

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        // ExecuteMsg::AddMinter { address, allowance } => execute
        ExecuteMsg::Mint { to_address, amount } => {
            execute_mint(deps, env, info, to_address, amount)
        } // ExecuteMsg::Burn { amount } => try_reset(deps, info, count),
        ExecuteMsg::Blacklist { address, status } => {
            execute_blacklist(deps, env, info, address, status)
        }
    }
}

pub fn execute_mint(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    to_address: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    deps.api.addr_validate(&to_address)?;
    let denom = query_denom(deps.as_ref())?;

    if amount.eq(&Uint128::new(0_u128)) {
        return Result::Err(ContractError::ZeroAmount {});
    }

    let allowance = MINTER_ALLOWANCES.update(
        deps.storage,
        info.sender,
        |allowance| -> StdResult<Uint128> {
            Ok(allowance.unwrap_or_default().checked_sub(amount)?)
        },
    )?;

    // TODO execute actual MintMsg
    let mint_tokens_msg =
        OsmosisMsg::mint_contract_tokens(denom, amount, env.contract.address.into_string());

    // TODO send tokens to to_address

    let res = Response::new()
        .add_attribute("method", "mint_tokens")
        .add_message(mint_tokens_msg);

    Ok(Response::new().add_attribute("method", "try_increment"))
}

pub fn execute_blacklist(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    address: String,
    status: bool,
) -> Result<Response, ContractError> {
    let addr = deps.api.addr_validate(&address)?;
    let denom = query_denom(deps.as_ref())?;

    let permitted = BLACKLISTER_ALLOWANCES.load(deps.storage, info.sender)?;

    if permitted == false {
        return Err(ContractError::Unauthorized {});
    }

    BLACKLISTED_ADDRESSES.update(deps.storage, addr, |current_status| -> StdResult<bool> {
        Ok(status)
    })?;

    Ok(Response::new().add_attribute("method", "blacklist"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn sudo(deps: DepsMut, _env: Env, msg: SudoMsg) -> Result<Response, ContractError> {
    match msg {
        SudoMsg::BeforeSend { from, to, amount } => beforesend_hook(deps, from, to, amount),
    }
}

pub fn beforesend_hook(
    deps: DepsMut,
    from: String,
    to: String,
    amount: Vec<Coin>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    let from_address = deps.api.addr_validate(&from)?;
    let to_address = deps.api.addr_validate(&to)?;

    if config.is_frozen {
        return Err(ContractError::Frozen {});
    }

    let from_blacklist_status = BLACKLISTED_ADDRESSES.may_load(deps.storage, from_address)?;

    Ok(Response::new().add_attribute("method", "beforesend_hook"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::IsFrozen {} => to_binary(&query_is_frozen(deps)?),
        QueryMsg::Denom {} => to_binary(&query_denom(deps)?),
    }
}

pub fn query_denom(deps: Deps) -> StdResult<String> {
    let config = CONFIG.load(deps.storage)?;
    return Ok(config.denom);
}

pub fn query_is_frozen(deps: Deps) -> StdResult<IsFrozenResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(IsFrozenResponse {
        is_frozen: config.is_frozen,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            subdenom: String::from("uusdc"),
        };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_binary(&res).unwrap();
        assert_eq!(17, value.count);
    }

    #[test]
    fn increment() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Increment {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // should increase counter by 1
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_binary(&res).unwrap();
        assert_eq!(18, value.count);
    }

    #[test]
    fn reset() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let unauth_info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let res = execute(deps.as_mut(), mock_env(), unauth_info, msg);
        match res {
            Err(ContractError::Unauthorized {}) => {}
            _ => panic!("Must return unauthorized error"),
        }

        // only the original creator can reset the counter
        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        // should now be 5
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_binary(&res).unwrap();
        assert_eq!(5, value.count);
    }
}
